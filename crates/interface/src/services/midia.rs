use anyhow::{Context, Result, bail};
use image::{RgbaImage, imageops};
use std::path::Path;
use windows::Win32::Media::MediaFoundation::{
    IMF2DBuffer, IMFAttributes, IMFSample, IMFSourceReader, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE,
    MF_MT_SUBTYPE, MF_PD_DURATION, MF_SOURCE_READER_ALL_STREAMS, MF_SOURCE_READER_DISABLE_DXVA,
    MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_SOURCE_READER_MEDIASOURCE,
    MF_SOURCE_READERF_ENDOFSTREAM, MFCreateAttributes, MFCreateMediaType, MFCreateSourceReaderFromURL,
    MFMediaType_Video, MFVideoFormat_RGB32,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Variant::VT_I8;
use windows::core::{GUID, HSTRING, Interface};

const TRILHA_VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
const TODAS_AS_TRILHAS: u32 = MF_SOURCE_READER_ALL_STREAMS.0 as u32;
const FONTE: u32 = MF_SOURCE_READER_MEDIASOURCE.0 as u32;
const POSICAO_EM_UNIDADES: GUID = GUID::zeroed();

pub fn variante_i64(valor: i64) -> PROPVARIANT {
    let mut variante = PROPVARIANT::default();
    unsafe {
        let interno = &mut *variante.Anonymous.Anonymous;
        interno.vt = VT_I8;
        interno.Anonymous.hVal = valor;
    }
    variante
}

pub fn ler_variante_i64(variante: &PROPVARIANT) -> i64 {
    unsafe { variante.Anonymous.Anonymous.Anonymous.hVal }
}

pub fn abrir_leitor(caminho: &Path, decodificar: bool) -> Result<IMFSourceReader> {
    let mut atributos: Option<IMFAttributes> = None;
    unsafe { MFCreateAttributes(&mut atributos, 2) }?;
    let atributos = atributos.context("atributos ausentes")?;
    if decodificar {
        unsafe {
            atributos.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)?;
            atributos.SetUINT32(&MF_SOURCE_READER_DISABLE_DXVA, 1)?;
        }
    }
    let url = HSTRING::from(caminho.as_os_str());
    unsafe { MFCreateSourceReaderFromURL(&url, &atributos) }.context("clipe ilegivel")
}

pub fn duracao_do_leitor(leitor: &IMFSourceReader) -> Result<i64> {
    let valor = unsafe { leitor.GetPresentationAttribute(FONTE, &MF_PD_DURATION) }?;
    Ok(ler_variante_i64(&valor))
}

pub fn posicionar_leitor(leitor: &IMFSourceReader, posicao: i64) -> Result<()> {
    unsafe { leitor.SetCurrentPosition(&POSICAO_EM_UNIDADES, &variante_i64(posicao)) }?;
    Ok(())
}

/// Decodifica quadros de um clipe para gerar miniaturas, sem tocar no arquivo original.
pub struct LeitorQuadros {
    leitor: IMFSourceReader,
    largura: u32,
    altura: u32,
    passo: i32,
    duracao: i64,
}

impl LeitorQuadros {
    pub fn abrir(caminho: &Path) -> Result<Self> {
        let leitor = abrir_leitor(caminho, true)?;
        let duracao = duracao_do_leitor(&leitor)?;
        unsafe {
            leitor.SetStreamSelection(TODAS_AS_TRILHAS, false)?;
            leitor.SetStreamSelection(TRILHA_VIDEO, true)?;
            let tipo = MFCreateMediaType()?;
            tipo.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            tipo.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
            leitor.SetCurrentMediaType(TRILHA_VIDEO, None, &tipo)?;
        }
        let atual = unsafe { leitor.GetCurrentMediaType(TRILHA_VIDEO) }?;
        let tamanho = unsafe { atual.GetUINT64(&MF_MT_FRAME_SIZE) }?;
        let (largura, altura) = ((tamanho >> 32) as u32, tamanho as u32);
        if largura == 0 || altura == 0 {
            bail!("clipe sem dimensoes validas");
        }
        let passo =
            unsafe { atual.GetUINT32(&MF_MT_DEFAULT_STRIDE) }.map_or(-(largura as i32 * 4), |valor| valor as i32);
        Ok(Self {
            leitor,
            largura,
            altura,
            passo,
            duracao,
        })
    }

    pub fn duracao(&self) -> i64 {
        self.duracao
    }

    pub fn quadro_em(&self, posicao: i64, largura_alvo: u32) -> Result<RgbaImage> {
        posicionar_leitor(&self.leitor, posicao)?;
        let mut sinalizadores = 0;
        let mut amostra = None;
        unsafe {
            self.leitor.ReadSample(
                TRILHA_VIDEO,
                0,
                None,
                Some(&mut sinalizadores),
                None,
                Some(&mut amostra),
            )
        }?;
        if sinalizadores & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            bail!("fim do clipe antes do quadro pedido");
        }
        let amostra = amostra.context("decodificador nao entregou quadro")?;
        let imagem = self.ler_imagem(&amostra)?;

        let altura_alvo = (u64::from(largura_alvo) * u64::from(self.altura) / u64::from(self.largura)) as u32;
        Ok(imageops::thumbnail(&imagem, largura_alvo, altura_alvo.max(1)))
    }

    fn ler_imagem(&self, amostra: &IMFSample) -> Result<RgbaImage> {
        let buffer = unsafe { amostra.GetBufferByIndex(0) }?;
        if let Ok(plano) = buffer.cast::<IMF2DBuffer>() {
            let mut inicio = std::ptr::null_mut();
            let mut passo = 0;
            unsafe { plano.Lock2D(&mut inicio, &mut passo) }?;
            let imagem = copiar_bgrx_para_rgba(inicio, passo, self.largura, self.altura);
            unsafe { plano.Unlock2D() }?;
            return Ok(imagem);
        }
        let continuo = unsafe { amostra.ConvertToContiguousBuffer() }?;
        let mut dados = std::ptr::null_mut();
        let mut tamanho = 0;
        unsafe { continuo.Lock(&mut dados, None, Some(&mut tamanho)) }?;
        let minimo = u64::from(self.passo.unsigned_abs()) * u64::from(self.altura);
        let imagem = if u64::from(tamanho) < minimo {
            None
        } else {
            let inicio = if self.passo < 0 {
                unsafe { dados.add(self.passo.unsigned_abs() as usize * (self.altura as usize - 1)) }
            } else {
                dados
            };
            Some(copiar_bgrx_para_rgba(inicio, self.passo, self.largura, self.altura))
        };
        unsafe { continuo.Unlock() }?;
        imagem.context("quadro menor que o esperado")
    }
}

fn copiar_bgrx_para_rgba(inicio: *const u8, passo: i32, largura: u32, altura: u32) -> RgbaImage {
    let mut imagem = RgbaImage::new(largura, altura);
    for (indice, linha) in imagem.chunks_exact_mut(largura as usize * 4).enumerate() {
        let origem = unsafe {
            let deslocamento = indice as isize * passo as isize;
            std::slice::from_raw_parts(inicio.offset(deslocamento), largura as usize * 4)
        };
        converter_linha(origem, linha);
    }
    imagem
}

fn converter_linha(origem: &[u8], destino: &mut [u8]) {
    for (pixel_origem, pixel_destino) in origem.chunks_exact(4).zip(destino.chunks_exact_mut(4)) {
        pixel_destino.copy_from_slice(&[pixel_origem[2], pixel_origem[1], pixel_origem[0], 255]);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn linha_troca_azul_com_vermelho_e_torna_opaco() {
        let mut destino = [0; 8];
        converter_linha(&[1, 2, 3, 0, 4, 5, 6, 0], &mut destino);
        assert_eq!(destino, [3, 2, 1, 255, 6, 5, 4, 255]);
    }

    #[test]
    fn variante_preserva_valor() {
        assert_eq!(ler_variante_i64(&variante_i64(1_234_567_890_123)), 1_234_567_890_123);
    }
}
