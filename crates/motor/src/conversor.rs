use crate::dispositivo::DispositivoGrafico;
use anyhow::{Context, Result};
use std::mem::ManuallyDrop;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_TEX2D_VPIV, D3D11_TEX2D_VPOV, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
    D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE, D3D11_VIDEO_PROCESSOR_CONTENT_DESC, D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC,
    D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0, D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC,
    D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0, D3D11_VIDEO_PROCESSOR_STREAM, D3D11_VIDEO_USAGE_OPTIMAL_SPEED,
    D3D11_VPIV_DIMENSION_TEXTURE2D, D3D11_VPOV_DIMENSION_TEXTURE2D, ID3D11Device, ID3D11Texture2D, ID3D11VideoContext1,
    ID3D11VideoDevice, ID3D11VideoProcessor, ID3D11VideoProcessorEnumerator, ID3D11VideoProcessorInputView,
    ID3D11VideoProcessorOutputView,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709, DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709, DXGI_FORMAT_NV12,
    DXGI_RATIONAL, DXGI_SAMPLE_DESC,
};
use windows::core::Interface;

const TEXTURAS_DE_SAIDA: usize = 4;
const LIMITE_VISOES_EM_CACHE: usize = 8;

struct SaidaNv12 {
    textura: ID3D11Texture2D,
    visao: ID3D11VideoProcessorOutputView,
}

/// Converte quadros BGRA em NV12 usando o bloco de vídeo da GPU, sem passar pela CPU.
pub struct ConversorNv12 {
    dispositivo_video: ID3D11VideoDevice,
    contexto_video: ID3D11VideoContext1,
    enumerador: ID3D11VideoProcessorEnumerator,
    processador: ID3D11VideoProcessor,
    saidas: Vec<SaidaNv12>,
    proxima_saida: usize,
    visoes_entrada: Vec<(ID3D11Texture2D, ID3D11VideoProcessorInputView)>,
    entrada: (u32, u32),
}

impl ConversorNv12 {
    pub fn new(grafico: &DispositivoGrafico, entrada: (u32, u32), saida: (u32, u32)) -> Result<Self> {
        let dispositivo_video: ID3D11VideoDevice = grafico.dispositivo.cast()?;
        let contexto_video: ID3D11VideoContext1 = grafico.contexto.cast()?;
        let descricao = descricao_conteudo(entrada, saida);
        let enumerador = unsafe { dispositivo_video.CreateVideoProcessorEnumerator(&descricao) }
            .context("processador de video indisponivel")?;
        let processador = unsafe { dispositivo_video.CreateVideoProcessor(&enumerador, 0) }?;

        unsafe {
            contexto_video.VideoProcessorSetStreamAutoProcessingMode(&processador, 0, false);
            contexto_video.VideoProcessorSetStreamColorSpace1(&processador, 0, DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709);
            contexto_video
                .VideoProcessorSetOutputColorSpace1(&processador, DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709);
        }

        let saidas = (0..TEXTURAS_DE_SAIDA)
            .map(|_| criar_saida(&grafico.dispositivo, &dispositivo_video, &enumerador, saida))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            dispositivo_video,
            contexto_video,
            enumerador,
            processador,
            saidas,
            proxima_saida: 0,
            visoes_entrada: Vec::new(),
            entrada,
        })
    }

    pub fn entrada(&self) -> (u32, u32) {
        self.entrada
    }

    pub fn converter(&mut self, textura_entrada: &ID3D11Texture2D) -> Result<ID3D11Texture2D> {
        let visao = self.visao_entrada(textura_entrada)?;
        let saida = &self.saidas[self.proxima_saida];
        let mut fluxo = D3D11_VIDEO_PROCESSOR_STREAM {
            Enable: true.into(),
            pInputSurface: ManuallyDrop::new(Some(visao)),
            ..Default::default()
        };
        let resultado = unsafe {
            self.contexto_video
                .VideoProcessorBlt(&self.processador, &saida.visao, 0, std::slice::from_ref(&fluxo))
        };
        unsafe { ManuallyDrop::drop(&mut fluxo.pInputSurface) };
        resultado.context("falha ao converter o quadro para NV12")?;

        let textura = saida.textura.clone();
        self.proxima_saida = (self.proxima_saida + 1) % self.saidas.len();
        Ok(textura)
    }

    fn visao_entrada(&mut self, textura: &ID3D11Texture2D) -> Result<ID3D11VideoProcessorInputView> {
        if let Some((_, visao)) = self.visoes_entrada.iter().find(|(existente, _)| existente == textura) {
            return Ok(visao.clone());
        }
        if self.visoes_entrada.len() >= LIMITE_VISOES_EM_CACHE {
            self.visoes_entrada.clear();
        }
        let descricao = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
            FourCC: 0,
            ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPIV {
                    MipSlice: 0,
                    ArraySlice: 0,
                },
            },
        };
        let mut visao = None;
        unsafe {
            self.dispositivo_video.CreateVideoProcessorInputView(
                textura,
                &self.enumerador,
                &descricao,
                Some(&mut visao),
            )
        }
        .context("falha ao criar a visao de entrada do processador")?;
        let visao = visao.context("visao de entrada ausente")?;
        self.visoes_entrada.push((textura.clone(), visao.clone()));
        Ok(visao)
    }
}

fn descricao_conteudo(entrada: (u32, u32), saida: (u32, u32)) -> D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
    let taxa = DXGI_RATIONAL {
        Numerator: 60,
        Denominator: 1,
    };
    D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
        InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
        InputFrameRate: taxa,
        InputWidth: entrada.0,
        InputHeight: entrada.1,
        OutputFrameRate: taxa,
        OutputWidth: saida.0,
        OutputHeight: saida.1,
        Usage: D3D11_VIDEO_USAGE_OPTIMAL_SPEED,
    }
}

fn criar_saida(
    dispositivo: &ID3D11Device,
    dispositivo_video: &ID3D11VideoDevice,
    enumerador: &ID3D11VideoProcessorEnumerator,
    tamanho: (u32, u32),
) -> Result<SaidaNv12> {
    let descricao = D3D11_TEXTURE2D_DESC {
        Width: tamanho.0,
        Height: tamanho.1,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_NV12,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut textura = None;
    unsafe { dispositivo.CreateTexture2D(&descricao, None, Some(&mut textura)) }
        .context("falha ao criar a textura NV12")?;
    let textura = textura.context("textura NV12 ausente")?;

    let descricao_visao = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
        ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
        Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 {
            Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 },
        },
    };
    let mut visao = None;
    unsafe {
        dispositivo_video.CreateVideoProcessorOutputView(&textura, enumerador, &descricao_visao, Some(&mut visao))
    }
    .context("falha ao criar a visao de saida do processador")?;
    Ok(SaidaNv12 {
        textura,
        visao: visao.context("visao de saida ausente")?,
    })
}
