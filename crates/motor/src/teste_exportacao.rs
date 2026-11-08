use crate::armazenamento::ArquivoCircular;
use crate::tipos_midia::{DescritorAudio, DescritorVideo, ler_blob};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::core::HSTRING;
use zaga_nucleo::UNIDADES_POR_SEGUNDO;
use zaga_nucleo::anel::{AnelPacotes, Fluxo};
use zaga_nucleo::autoclipagem::Destaque;
use zaga_nucleo::config::Codec;

#[test]
#[ignore = "Requer ZAGA_VIDEO_SINTETICO com vídeo de barras e áudio gerado, sem acesso à tela"]
fn exporta_intervalo_historico_com_audio_e_nome() -> Result<()> {
    let origem = PathBuf::from(std::env::var_os("ZAGA_VIDEO_SINTETICO").context("Vídeo sintético ausente")?);
    let pasta = origem.parent().context("Pasta ausente")?;
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
    unsafe { MFStartup(MF_VERSION, 0) }?;
    let leitor = unsafe { MFCreateSourceReaderFromURL(&HSTRING::from(origem.as_os_str()), None) }?;
    let tipo = unsafe { leitor.GetNativeMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, 0) }?;
    let tamanho = unsafe { tipo.GetUINT64(&MF_MT_FRAME_SIZE) }?;
    let taxa = unsafe { tipo.GetUINT64(&MF_MT_FRAME_RATE) }?;
    let video = DescritorVideo {
        codec: Codec::H264,
        largura: (tamanho >> 32) as u32,
        altura: tamanho as u32,
        fps: ((taxa >> 32) / (taxa as u32).max(1) as u64) as u32,
        bitrate: 2_000_000,
        cabecalho: ler_blob(&tipo, &MF_MT_MPEG_SEQUENCE_HEADER).context("Cabeçalho de vídeo ausente")?,
    };
    let tipo_audio = unsafe { leitor.GetNativeMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32, 0) }?;
    let audio = DescritorAudio {
        dados_codec: ler_blob(&tipo_audio, &MF_MT_USER_DATA).unwrap_or_default(),
    };
    let caminho = pasta.join(format!("teste_exportacao_{}.bin", std::process::id()));
    let anel = Arc::new(Mutex::new(AnelPacotes::new(
        ArquivoCircular::criar(&caminho, 16 * 1024 * 1024)?,
        60 * UNIDADES_POR_SEGUNDO,
    )));
    let base = crate::tempo::unix_ms();
    let deslocamento = crate::tempo::de_unix_ms(base);
    for (fluxo, trilha) in [
        (Fluxo::Video, MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32),
        (Fluxo::Audio, MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32),
    ] {
        let leitor = unsafe { MFCreateSourceReaderFromURL(&HSTRING::from(origem.as_os_str()), None) }?;
        loop {
            let mut flags = 0;
            let mut amostra = None;
            unsafe { leitor.ReadSample(trilha, 0, None, Some(&mut flags), None, Some(&mut amostra)) }?;
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                break;
            }
            let Some(amostra) = amostra else {
                continue;
            };
            let pts = unsafe { amostra.GetSampleTime() }? + deslocamento;
            let chave = unsafe { amostra.GetUINT32(&MFSampleExtension_CleanPoint) }.unwrap_or(0) != 0;
            let buffer = unsafe { amostra.ConvertToContiguousBuffer() }?;
            let mut ponteiro = std::ptr::null_mut();
            let mut tamanho = 0;
            unsafe { buffer.Lock(&mut ponteiro, None, Some(&mut tamanho)) }?;
            let bytes = unsafe { std::slice::from_raw_parts(ponteiro, tamanho as usize) }.to_vec();
            unsafe { buffer.Unlock() }?;
            anel.lock().unwrap().inserir(fluxo, pts, chave, &bytes)?;
        }
    }
    let destaque = Destaque {
        titulo: "4K: Ancient".into(),
        jogo: "Counter Strike 2".into(),
        partida: "teste".into(),
        rodada: "1".into(),
        inicio_ms: base + 2000,
        fim_ms: base + 5000,
        eliminacoes: 4,
        mapa: "Ancient".into(),
        vitoria: false,
    };
    let destino = crate::gravador_clipe::salvar_destaque(&anel, &video, Some(&audio), &destaque, pasta)?;
    let metadata: Destaque = serde_json::from_slice(&std::fs::read(destino.with_extension("json"))?)?;
    assert_eq!(metadata, destaque);
    assert!(
        destino
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .starts_with("4K Ancient ")
    );
    println!("Clipe sintético exportado: {}", destino.display());
    drop(anel);
    assert!(!caminho.exists());
    unsafe { MFShutdown() }?;
    Ok(())
}
