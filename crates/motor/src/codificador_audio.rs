use crate::tipos_midia::{DescritorAudio, ler_blob, tipo_audio_aac, tipo_audio_pcm};
use anyhow::{Context, Result, anyhow};
use std::mem::ManuallyDrop;
use windows::Win32::Media::MediaFoundation::{
    IMFSample, IMFTransform, MF_E_TRANSFORM_NEED_MORE_INPUT, MF_MT_USER_DATA, MFCreateMemoryBuffer, MFCreateSample,
    MFT_MESSAGE_COMMAND_FLUSH, MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, MFT_MESSAGE_NOTIFY_START_OF_STREAM,
    MFT_OUTPUT_DATA_BUFFER,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::core::GUID;

const CLSID_ENCODER_AAC: GUID = GUID::from_u128(0x93af0c51_2275_45d2_a35b_f2ba21caed00);

/// Encoder AAC em software do Windows, de uso de CPU desprezível frente ao de vídeo.
pub struct CodificadorAudio {
    transformacao: IMFTransform,
    amostra_saida: IMFSample,
    descritor: DescritorAudio,
}

impl CodificadorAudio {
    pub fn new() -> Result<Self> {
        let transformacao: IMFTransform = unsafe { CoCreateInstance(&CLSID_ENCODER_AAC, None, CLSCTX_INPROC_SERVER) }
            .context("encoder AAC indisponivel")?;
        unsafe {
            transformacao
                .SetOutputType(0, &tipo_audio_aac()?, 0)
                .context("encoder AAC recusou a saida")?;
            transformacao
                .SetInputType(0, &tipo_audio_pcm()?, 0)
                .context("encoder AAC recusou a entrada")?;
        }
        let tipo = unsafe { transformacao.GetOutputCurrentType(0) }?;
        let dados_codec = ler_blob(&tipo, &MF_MT_USER_DATA).context("encoder AAC sem dados de configuracao")?;

        let informacao = unsafe { transformacao.GetOutputStreamInfo(0) }?;
        let amostra_saida = unsafe { MFCreateSample() }?;
        unsafe {
            amostra_saida.AddBuffer(&MFCreateMemoryBuffer(informacao.cbSize)?)?;
            transformacao.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0)?;
            transformacao.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            transformacao.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
        }
        Ok(Self {
            transformacao,
            amostra_saida,
            descritor: DescritorAudio { dados_codec },
        })
    }

    pub fn descritor(&self) -> &DescritorAudio {
        &self.descritor
    }

    pub fn codificar(
        &self,
        pcm: &[u8],
        pts: i64,
        duracao: i64,
        mut consumir: impl FnMut(&[u8], i64) -> Result<()>,
    ) -> Result<()> {
        let entrada = criar_amostra_entrada(pcm, pts, duracao)?;
        unsafe { self.transformacao.ProcessInput(0, &entrada, 0) }?;
        while self.retirar_pacote(&mut consumir)? {}
        Ok(())
    }

    fn retirar_pacote(&self, consumir: &mut impl FnMut(&[u8], i64) -> Result<()>) -> Result<bool> {
        let mut saida = [MFT_OUTPUT_DATA_BUFFER {
            dwStreamID: 0,
            pSample: ManuallyDrop::new(Some(self.amostra_saida.clone())),
            dwStatus: 0,
            pEvents: ManuallyDrop::new(None),
        }];
        let mut status = 0;
        let resultado = unsafe { self.transformacao.ProcessOutput(0, &mut saida, &mut status) };
        unsafe {
            ManuallyDrop::drop(&mut saida[0].pSample);
            ManuallyDrop::drop(&mut saida[0].pEvents);
        }
        match resultado {
            Err(erro) if erro.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(false),
            Err(erro) => return Err(anyhow!(erro).context("falha ao obter pacote AAC")),
            Ok(()) => {}
        }

        let pts = unsafe { self.amostra_saida.GetSampleTime() }?;
        let buffer = unsafe { self.amostra_saida.ConvertToContiguousBuffer() }?;
        let mut ponteiro = std::ptr::null_mut();
        let mut tamanho = 0;
        unsafe { buffer.Lock(&mut ponteiro, None, Some(&mut tamanho)) }?;
        let dados = unsafe { std::slice::from_raw_parts(ponteiro, tamanho as usize) };
        let consumido = consumir(dados, pts);
        unsafe {
            buffer.Unlock()?;
            buffer.SetCurrentLength(0)?;
        }
        consumido?;
        Ok(true)
    }
}

fn criar_amostra_entrada(pcm: &[u8], pts: i64, duracao: i64) -> Result<IMFSample> {
    let buffer = unsafe { MFCreateMemoryBuffer(pcm.len() as u32) }?;
    let mut ponteiro = std::ptr::null_mut();
    unsafe {
        buffer.Lock(&mut ponteiro, None, None)?;
        std::ptr::copy_nonoverlapping(pcm.as_ptr(), ponteiro, pcm.len());
        buffer.Unlock()?;
        buffer.SetCurrentLength(pcm.len() as u32)?;
    }
    let amostra = unsafe { MFCreateSample() }?;
    unsafe {
        amostra.AddBuffer(&buffer)?;
        amostra.SetSampleTime(pts)?;
        amostra.SetSampleDuration(duracao)?;
    }
    Ok(amostra)
}
