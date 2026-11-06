use anyhow::Result;
use windows::Win32::Media::MediaFoundation::{
    IMFMediaType, MF_MT_AAC_PAYLOAD_TYPE, MF_MT_ALL_SAMPLES_INDEPENDENT, MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
    MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_BLOCK_ALIGNMENT, MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND,
    MF_MT_AVG_BITRATE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE,
    MF_MT_MPEG_SEQUENCE_HEADER, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE, MF_MT_TRANSFER_FUNCTION, MF_MT_USER_DATA,
    MF_MT_VIDEO_NOMINAL_RANGE, MF_MT_VIDEO_PRIMARIES, MF_MT_YUV_MATRIX, MFAudioFormat_AAC, MFAudioFormat_PCM,
    MFCreateMediaType, MFMediaType_Audio, MFMediaType_Video, MFNominalRange_16_235, MFVideoFormat_H264,
    MFVideoFormat_HEVC, MFVideoInterlace_Progressive, MFVideoPrimaries_BT709, MFVideoTransFunc_709,
    MFVideoTransferMatrix_BT709,
};
use windows::core::GUID;
use zaga_nucleo::config::{BITRATE_AUDIO, Codec};

pub const TAXA_AUDIO: u32 = 48_000;
pub const CANAIS_AUDIO: u32 = 2;
pub const BYTES_POR_QUADRO_AUDIO: u32 = CANAIS_AUDIO * 2;
pub const QUADROS_POR_PACOTE_AAC: u32 = 1024;

/// Dados necessários para descrever a trilha de vídeo, sem manter objetos COM vivos entre threads.
#[derive(Clone, Debug)]
pub struct DescritorVideo {
    pub codec: Codec,
    pub largura: u32,
    pub altura: u32,
    pub fps: u32,
    pub bitrate: u32,
    pub cabecalho: Vec<u8>,
}

/// Dados necessários para descrever a trilha de áudio AAC.
#[derive(Clone, Debug)]
pub struct DescritorAudio {
    pub dados_codec: Vec<u8>,
}

pub fn subtipo_do_codec(codec: Codec) -> GUID {
    match codec {
        Codec::H264 => MFVideoFormat_H264,
        Codec::Hevc => MFVideoFormat_HEVC,
    }
}

pub fn empacotar_par(alto: u32, baixo: u32) -> u64 {
    (u64::from(alto) << 32) | u64::from(baixo)
}

pub fn definir_geometria_video(tipo: &IMFMediaType, largura: u32, altura: u32, fps: u32) -> Result<()> {
    unsafe {
        tipo.SetUINT64(&MF_MT_FRAME_SIZE, empacotar_par(largura, altura))?;
        tipo.SetUINT64(&MF_MT_FRAME_RATE, empacotar_par(fps, 1))?;
        tipo.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, empacotar_par(1, 1))?;
        tipo.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        tipo.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32)?;
        tipo.SetUINT32(&MF_MT_VIDEO_PRIMARIES, MFVideoPrimaries_BT709.0 as u32)?;
        tipo.SetUINT32(&MF_MT_TRANSFER_FUNCTION, MFVideoTransFunc_709.0 as u32)?;
        tipo.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32)?;
    }
    Ok(())
}

pub fn tipo_video_comprimido(codec: Codec, largura: u32, altura: u32, fps: u32, bitrate: u32) -> Result<IMFMediaType> {
    let tipo = unsafe { MFCreateMediaType() }?;
    unsafe {
        tipo.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        tipo.SetGUID(&MF_MT_SUBTYPE, &subtipo_do_codec(codec))?;
        tipo.SetUINT32(&MF_MT_AVG_BITRATE, bitrate)?;
    }
    definir_geometria_video(&tipo, largura, altura, fps)?;
    Ok(tipo)
}

pub fn tipo_video_do_descritor(descritor: &DescritorVideo) -> Result<IMFMediaType> {
    let tipo = tipo_video_comprimido(
        descritor.codec,
        descritor.largura,
        descritor.altura,
        descritor.fps,
        descritor.bitrate,
    )?;
    unsafe { tipo.SetBlob(&MF_MT_MPEG_SEQUENCE_HEADER, &descritor.cabecalho) }?;
    Ok(tipo)
}

pub fn tipo_audio_pcm() -> Result<IMFMediaType> {
    let tipo = tipo_audio_base(&MFAudioFormat_PCM)?;
    unsafe {
        tipo.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, BYTES_POR_QUADRO_AUDIO)?;
        tipo.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, TAXA_AUDIO * BYTES_POR_QUADRO_AUDIO)?;
        tipo.SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1)?;
    }
    Ok(tipo)
}

pub fn tipo_audio_aac() -> Result<IMFMediaType> {
    let tipo = tipo_audio_base(&MFAudioFormat_AAC)?;
    unsafe {
        tipo.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, BITRATE_AUDIO / 8)?;
        tipo.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0)?;
    }
    Ok(tipo)
}

pub fn tipo_audio_do_descritor(descritor: &DescritorAudio) -> Result<IMFMediaType> {
    let tipo = tipo_audio_aac()?;
    unsafe { tipo.SetBlob(&MF_MT_USER_DATA, &descritor.dados_codec) }?;
    Ok(tipo)
}

pub fn ler_blob(tipo: &IMFMediaType, chave: &GUID) -> Option<Vec<u8>> {
    let tamanho = unsafe { tipo.GetBlobSize(chave) }.ok()?;
    let mut dados = vec![0; tamanho as usize];
    unsafe { tipo.GetBlob(chave, &mut dados, None) }.ok()?;
    (!dados.is_empty()).then_some(dados)
}

fn tipo_audio_base(subtipo: &GUID) -> Result<IMFMediaType> {
    let tipo = unsafe { MFCreateMediaType() }?;
    unsafe {
        tipo.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        tipo.SetGUID(&MF_MT_SUBTYPE, subtipo)?;
        tipo.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
        tipo.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, TAXA_AUDIO)?;
        tipo.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, CANAIS_AUDIO)?;
    }
    Ok(tipo)
}
