use crate::dispositivo::DispositivoGrafico;
use crate::registro::registrar;
use crate::tipos_midia::{definir_geometria_video, ler_blob, subtipo_do_codec, tipo_video_comprimido};
use anyhow::{Context, Result, anyhow, bail};
use std::mem::ManuallyDrop;
use windows::Win32::Foundation::LUID;
use windows::Win32::Foundation::VARIANT_TRUE;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Media::MediaFoundation::{
    CODECAPI_AVEncCommonMaxBitRate, CODECAPI_AVEncCommonMeanBitRate, CODECAPI_AVEncCommonQualityVsSpeed,
    CODECAPI_AVEncCommonRateControlMode, CODECAPI_AVEncMPVDefaultBPictureCount, CODECAPI_AVEncMPVGOPSize,
    CODECAPI_AVEncVideoForceKeyFrame, CODECAPI_AVLowLatencyMode, ICodecAPI, IMF2DBuffer, IMFActivate, IMFAttributes,
    IMFDXGIDeviceManager, IMFMediaEventGenerator, IMFSample, IMFTransform, MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS,
    METransformHaveOutput, METransformNeedInput, MF_E_TRANSFORM_NEED_MORE_INPUT, MF_E_TRANSFORM_STREAM_CHANGE,
    MF_MT_MPEG_SEQUENCE_HEADER, MF_MT_SUBTYPE, MF_TRANSFORM_ASYNC_UNLOCK, MFCreateAttributes,
    MFCreateDXGIDeviceManager, MFCreateDXGISurfaceBuffer, MFCreateMemoryBuffer, MFCreateSample, MFMediaType_Video,
    MFSampleExtension_CleanPoint, MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_ADAPTER_LUID, MFT_ENUM_FLAG_HARDWARE,
    MFT_ENUM_FLAG_SORTANDFILTER, MFT_FRIENDLY_NAME_Attribute, MFT_MESSAGE_COMMAND_FLUSH,
    MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, MFT_MESSAGE_NOTIFY_END_STREAMING, MFT_MESSAGE_NOTIFY_START_OF_STREAM,
    MFT_MESSAGE_SET_D3D_MANAGER, MFT_OUTPUT_DATA_BUFFER, MFT_OUTPUT_STREAM_PROVIDES_SAMPLES, MFT_REGISTER_TYPE_INFO,
    MFT_TRANSFORM_CLSID_Attribute, MFTEnum2, MFVideoFormat_NV12, eAVEncCommonRateControlMode_PeakConstrainedVBR,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::System::Variant::{VARIANT, VT_BOOL, VT_UI4};
use windows::core::{GUID, Interface, PWSTR};
use zaga_nucleo::config::Codec;

const QUALIDADE_VS_VELOCIDADE: u32 = 70;
const GOP_SEGUNDOS: u32 = 2;
const FATOR_BITRATE_PICO: u32 = 2;

pub struct ParametrosVideo {
    pub codec: Codec,
    pub largura: u32,
    pub altura: u32,
    pub fps: u32,
    pub bitrate: u32,
}

pub enum EventoCodificador {
    PrecisaEntrada,
    TemSaida,
    Outro,
}

pub struct PacoteVideo<'a> {
    pub dados: &'a [u8],
    pub pts: i64,
    pub chave: bool,
}

/// Encoder de hardware assíncrono do Media Foundation, alimentado diretamente por texturas da GPU.
pub struct CodificadorVideo {
    ativacao: Option<IMFActivate>,
    transformacao: IMFTransform,
    eventos: IMFMediaEventGenerator,
    api_codec: Option<ICodecAPI>,
    _gerenciador: IMFDXGIDeviceManager,
    nome: String,
    tamanho_amostra_saida: u32,
    fornece_amostras: bool,
}

impl CodificadorVideo {
    pub fn new(grafico: &DispositivoGrafico, parametros: &ParametrosVideo) -> Result<Self> {
        let ativacao = encontrar_encoder(parametros.codec, grafico.luid)?;
        let nome = nome_amigavel(&ativacao);
        let (transformacao, ativacao) =
            instanciar_pelo_clsid_ou_ativacao(ativacao).with_context(|| format!("falha ao ativar {nome}"))?;
        unsafe { transformacao.GetAttributes()?.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1) }?;

        let gerenciador = conectar_dispositivo(&transformacao, grafico)?;
        let api_codec = transformacao.cast::<ICodecAPI>().ok();
        if let Some(api) = &api_codec {
            configurar_controle_de_taxa(api, parametros);
        }

        let tipo_saida = tipo_video_comprimido(
            parametros.codec,
            parametros.largura,
            parametros.altura,
            parametros.fps,
            parametros.bitrate,
        )?;
        unsafe { transformacao.SetOutputType(0, &tipo_saida, 0) }.context("encoder recusou o tipo de saida")?;
        definir_tipo_entrada(&transformacao, parametros)?;

        let informacao = unsafe { transformacao.GetOutputStreamInfo(0) }?;
        let fornece_amostras = informacao.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 != 0;
        let eventos: IMFMediaEventGenerator = transformacao.cast()?;

        unsafe {
            transformacao.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0)?;
            transformacao.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            transformacao.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
        }

        Ok(Self {
            ativacao,
            transformacao,
            eventos,
            api_codec,
            _gerenciador: gerenciador,
            nome,
            tamanho_amostra_saida: informacao.cbSize,
            fornece_amostras,
        })
    }

    pub fn nome(&self) -> &str {
        &self.nome
    }

    pub fn cabecalho_do_tipo(&self) -> Option<Vec<u8>> {
        let tipo = unsafe { self.transformacao.GetOutputCurrentType(0) }.ok()?;
        ler_blob(&tipo, &MF_MT_MPEG_SEQUENCE_HEADER)
    }

    pub fn proximo_evento(&self) -> Result<EventoCodificador> {
        let evento = unsafe { self.eventos.GetEvent(MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS(0)) }?;
        let tipo = unsafe { evento.GetType() }?;
        let status = unsafe { evento.GetStatus() }?;
        status
            .ok()
            .with_context(|| format!("encoder sinalizou falha no evento {tipo}"))?;
        Ok(match tipo {
            valor if valor == METransformNeedInput.0 as u32 => EventoCodificador::PrecisaEntrada,
            valor if valor == METransformHaveOutput.0 as u32 => EventoCodificador::TemSaida,
            outro => {
                registrar(format!("evento do encoder ignorado: {outro}"));
                EventoCodificador::Outro
            }
        })
    }

    pub fn forcar_quadro_chave(&self) {
        if let Some(api) = &self.api_codec {
            definir_opcional(
                api,
                &CODECAPI_AVEncVideoForceKeyFrame,
                variante_u32(1),
                "quadro chave forcado",
            );
        }
    }

    pub fn enviar(&self, textura: &ID3D11Texture2D, pts: i64, duracao: i64) -> Result<()> {
        let buffer = unsafe { MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, textura, 0, false) }?;
        let tamanho = unsafe { buffer.cast::<IMF2DBuffer>()?.GetContiguousLength() }?;
        unsafe { buffer.SetCurrentLength(tamanho) }?;
        let amostra = unsafe { MFCreateSample() }?;
        unsafe {
            amostra.AddBuffer(&buffer)?;
            amostra.SetSampleTime(pts)?;
            amostra.SetSampleDuration(duracao)?;
            self.transformacao.ProcessInput(0, &amostra, 0)?;
        }
        Ok(())
    }

    /// Retira um pacote do encoder, entregando seus bytes sem cópia intermediária.
    pub fn receber(&self, mut consumir: impl FnMut(PacoteVideo<'_>) -> Result<()>) -> Result<()> {
        let amostra_propria = self.criar_amostra_saida()?;
        let mut saida = [MFT_OUTPUT_DATA_BUFFER {
            dwStreamID: 0,
            pSample: ManuallyDrop::new(amostra_propria),
            dwStatus: 0,
            pEvents: ManuallyDrop::new(None),
        }];
        let mut status = 0;
        let resultado = unsafe { self.transformacao.ProcessOutput(0, &mut saida, &mut status) };
        let amostra = unsafe { ManuallyDrop::take(&mut saida[0].pSample) };
        drop(unsafe { ManuallyDrop::take(&mut saida[0].pEvents) });

        match resultado {
            Err(erro) if erro.code() == MF_E_TRANSFORM_STREAM_CHANGE => return self.renegociar_saida(),
            Err(erro) if erro.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(()),
            Err(erro) => return Err(anyhow!(erro).context("falha ao obter pacote do encoder")),
            Ok(()) => {}
        }
        let amostra = amostra.context("encoder nao entregou amostra")?;
        entregar_amostra(&amostra, &mut consumir)
    }

    fn criar_amostra_saida(&self) -> Result<Option<IMFSample>> {
        if self.fornece_amostras {
            return Ok(None);
        }
        let amostra = unsafe { MFCreateSample() }?;
        let buffer = unsafe { MFCreateMemoryBuffer(self.tamanho_amostra_saida) }?;
        unsafe { amostra.AddBuffer(&buffer) }?;
        Ok(Some(amostra))
    }

    fn renegociar_saida(&self) -> Result<()> {
        let tipo = unsafe { self.transformacao.GetOutputAvailableType(0, 0) }?;
        unsafe { self.transformacao.SetOutputType(0, &tipo, 0) }?;
        Ok(())
    }
}

impl Drop for CodificadorVideo {
    fn drop(&mut self) {
        unsafe {
            let _ = self.transformacao.ProcessMessage(MFT_MESSAGE_NOTIFY_END_STREAMING, 0);
            let _ = self.transformacao.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
            if let Some(ativacao) = &self.ativacao {
                let _ = ativacao.ShutdownObject();
            }
        }
    }
}

fn encontrar_encoder(codec: Codec, luid: LUID) -> Result<IMFActivate> {
    let saida = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: subtipo_do_codec(codec),
    };
    let mut atributos: Option<IMFAttributes> = None;
    unsafe { MFCreateAttributes(&mut atributos, 1) }?;
    let atributos = atributos.context("atributos ausentes")?;
    let bytes_luid = unsafe { std::slice::from_raw_parts((&raw const luid).cast::<u8>(), size_of::<LUID>()) };
    unsafe { atributos.SetBlob(&MFT_ENUM_ADAPTER_LUID, bytes_luid) }?;

    let mut lista = std::ptr::null_mut();
    let mut quantidade = 0;
    unsafe {
        MFTEnum2(
            MFT_CATEGORY_VIDEO_ENCODER,
            MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
            None,
            Some(&saida),
            &atributos,
            &mut lista,
            &mut quantidade,
        )
    }?;
    let primeiro = primeira_ativacao(lista, quantidade);
    primeiro.with_context(|| format!("nenhum encoder de hardware para {codec:?} neste adaptador"))
}

fn instanciar_pelo_clsid_ou_ativacao(ativacao: IMFActivate) -> Result<(IMFTransform, Option<IMFActivate>)> {
    if let Ok(clsid) = unsafe { ativacao.GetGUID(&MFT_TRANSFORM_CLSID_Attribute) } {
        if let Ok(transformacao) = unsafe { CoCreateInstance(&clsid, None, CLSCTX_INPROC_SERVER) } {
            return Ok((transformacao, None));
        }
    }
    let transformacao = unsafe { ativacao.ActivateObject() }?;
    Ok((transformacao, Some(ativacao)))
}

fn primeira_ativacao(lista: *mut Option<IMFActivate>, quantidade: u32) -> Option<IMFActivate> {
    if lista.is_null() {
        return None;
    }
    let itens = unsafe { std::slice::from_raw_parts_mut(lista, quantidade as usize) };
    let primeiro = itens.iter_mut().find_map(Option::take);
    itens.iter_mut().for_each(|item| drop(item.take()));
    unsafe { CoTaskMemFree(Some(lista.cast())) };
    primeiro
}

fn nome_amigavel(ativacao: &IMFActivate) -> String {
    let mut texto = PWSTR::null();
    let mut tamanho = 0;
    if unsafe { ativacao.GetAllocatedString(&MFT_FRIENDLY_NAME_Attribute, &mut texto, &mut tamanho) }.is_err() {
        return String::from("encoder de hardware");
    }
    let nome = unsafe { texto.to_string() }.unwrap_or_default();
    unsafe { CoTaskMemFree(Some(texto.0.cast())) };
    nome
}

fn conectar_dispositivo(transformacao: &IMFTransform, grafico: &DispositivoGrafico) -> Result<IMFDXGIDeviceManager> {
    let mut token = 0;
    let mut gerenciador = None;
    unsafe { MFCreateDXGIDeviceManager(&mut token, &mut gerenciador) }?;
    let gerenciador = gerenciador.context("gerenciador DXGI ausente")?;
    unsafe {
        gerenciador.ResetDevice(&grafico.dispositivo, token)?;
        transformacao.ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER, gerenciador.as_raw() as usize)
    }
    .context("encoder recusou o dispositivo D3D11")?;
    Ok(gerenciador)
}

fn configurar_controle_de_taxa(api: &ICodecAPI, parametros: &ParametrosVideo) {
    let modo = eAVEncCommonRateControlMode_PeakConstrainedVBR.0 as u32;
    let gop = parametros.fps * GOP_SEGUNDOS;
    let pico = parametros.bitrate.saturating_mul(FATOR_BITRATE_PICO);
    definir_opcional(api, &CODECAPI_AVLowLatencyMode, variante_verdadeira(), "baixa latencia");
    definir_opcional(
        api,
        &CODECAPI_AVEncCommonRateControlMode,
        variante_u32(modo),
        "controle de taxa",
    );
    definir_opcional(
        api,
        &CODECAPI_AVEncCommonMeanBitRate,
        variante_u32(parametros.bitrate),
        "bitrate medio",
    );
    definir_opcional(
        api,
        &CODECAPI_AVEncCommonMaxBitRate,
        variante_u32(pico),
        "bitrate de pico",
    );
    definir_opcional(api, &CODECAPI_AVEncMPVGOPSize, variante_u32(gop), "tamanho do GOP");
    definir_opcional(
        api,
        &CODECAPI_AVEncMPVDefaultBPictureCount,
        variante_u32(0),
        "quadros B",
    );
    definir_opcional(
        api,
        &CODECAPI_AVEncCommonQualityVsSpeed,
        variante_u32(QUALIDADE_VS_VELOCIDADE),
        "qualidade",
    );
}

fn variante_u32(valor: u32) -> VARIANT {
    let mut variante = VARIANT::default();
    unsafe {
        let interno = &mut *variante.Anonymous.Anonymous;
        interno.vt = VT_UI4;
        interno.Anonymous.ulVal = valor;
    }
    variante
}

fn variante_verdadeira() -> VARIANT {
    let mut variante = VARIANT::default();
    unsafe {
        let interno = &mut *variante.Anonymous.Anonymous;
        interno.vt = VT_BOOL;
        interno.Anonymous.boolVal = VARIANT_TRUE;
    }
    variante
}

fn definir_opcional(api: &ICodecAPI, chave: &GUID, valor: VARIANT, descricao: &str) {
    if let Err(erro) = unsafe { api.SetValue(chave, &valor) } {
        registrar(format!("encoder ignorou {descricao}: {erro}"));
    }
}

fn definir_tipo_entrada(transformacao: &IMFTransform, parametros: &ParametrosVideo) -> Result<()> {
    for indice in 0.. {
        let Ok(tipo) = (unsafe { transformacao.GetInputAvailableType(0, indice) }) else {
            break;
        };
        if unsafe { tipo.GetGUID(&MF_MT_SUBTYPE) }? != MFVideoFormat_NV12 {
            continue;
        }
        definir_geometria_video(&tipo, parametros.largura, parametros.altura, parametros.fps)?;
        unsafe { transformacao.SetInputType(0, &tipo, 0) }.context("encoder recusou a entrada NV12")?;
        return Ok(());
    }
    bail!("encoder nao aceita NV12")
}

fn entregar_amostra(amostra: &IMFSample, consumir: &mut impl FnMut(PacoteVideo<'_>) -> Result<()>) -> Result<()> {
    let pts = unsafe { amostra.GetSampleTime() }?;
    let chave = unsafe { amostra.GetUINT32(&MFSampleExtension_CleanPoint) }.unwrap_or(0) != 0;
    let buffer = unsafe { amostra.ConvertToContiguousBuffer() }?;
    let mut ponteiro = std::ptr::null_mut();
    let mut tamanho = 0;
    unsafe { buffer.Lock(&mut ponteiro, None, Some(&mut tamanho)) }?;
    let dados = unsafe { std::slice::from_raw_parts(ponteiro, tamanho as usize) };
    let resultado = consumir(PacoteVideo { dados, pts, chave });
    unsafe { buffer.Unlock() }?;
    resultado
}
