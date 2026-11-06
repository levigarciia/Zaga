//! Decodifica um clipe por completo e imprime métricas objetivas, sem exibir o conteúdo do vídeo.
//!
//! Uso: `cargo run -p zaga_motor --example verificar_clipe -- caminho.mp4`

use anyhow::{Context, Result, bail};
use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFSample, IMFSourceReader, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
    MF_SOURCE_READER_DISABLE_DXVA, MF_SOURCE_READER_FIRST_AUDIO_STREAM, MF_SOURCE_READER_FIRST_VIDEO_STREAM,
    MF_SOURCE_READERF_ENDOFSTREAM, MF_VERSION, MFAudioFormat_PCM, MFCreateAttributes, MFCreateMediaType,
    MFCreateSourceReaderFromURL, MFMediaType_Audio, MFMediaType_Video, MFSampleExtension_CleanPoint, MFShutdown,
    MFStartup, MFVideoFormat_NV12,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::core::{GUID, HSTRING};

const TRILHA_VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
const TRILHA_AUDIO: u32 = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
const UNIDADES_POR_SEGUNDO: f64 = 10_000_000.0;

#[derive(Default)]
struct MetricasVideo {
    quadros: u64,
    primeiro_pts: Option<i64>,
    ultimo_pts: i64,
    maior_intervalo: i64,
    soma_luma: f64,
    amostras_luma: u64,
    luma_minima: u8,
    luma_maxima: u8,
    soma_quadrados_luma: f64,
}

#[derive(Default)]
struct MetricasAudio {
    blocos: u64,
    primeiro_pts: Option<i64>,
    ultimo_pts: i64,
    bytes: u64,
    pico: i32,
}

fn main() -> Result<()> {
    let caminho = std::env::args().nth(1).context("informe o caminho do clipe")?;
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        MFStartup(MF_VERSION, 0)?;
    }
    let resultado = verificar(&caminho);
    let _ = unsafe { MFShutdown() };
    resultado
}

fn verificar(caminho: &str) -> Result<()> {
    let tamanho_arquivo = std::fs::metadata(caminho)?.len();
    imprimir_conteiner(caminho)?;
    let leitor = abrir_leitor(caminho)?;

    let nativo = unsafe { leitor.GetNativeMediaType(TRILHA_VIDEO, 0) }.context("sem trilha de video")?;
    let subtipo_video = unsafe { nativo.GetGUID(&MF_MT_SUBTYPE) }?;
    let tamanho = unsafe { nativo.GetUINT64(&MF_MT_FRAME_SIZE) }?;
    let (largura, altura) = ((tamanho >> 32) as usize, (tamanho & 0xFFFF_FFFF) as usize);
    println!("video: {} {largura}x{altura}", nome_fourcc(&subtipo_video));

    pedir_saida(&leitor, TRILHA_VIDEO, &MFMediaType_Video, &MFVideoFormat_NV12)?;
    let tem_audio = pedir_saida(&leitor, TRILHA_AUDIO, &MFMediaType_Audio, &MFAudioFormat_PCM).is_ok();
    if !tem_audio {
        println!("audio: ausente");
    }

    let video = ler_video(&leitor, largura, altura)?;
    imprimir_video(&video, tamanho_arquivo);
    if tem_audio {
        let audio = ler_audio(&leitor)?;
        imprimir_audio(&audio, &video);
    }
    Ok(())
}

fn abrir_leitor(caminho: &str) -> Result<IMFSourceReader> {
    let mut atributos: Option<IMFAttributes> = None;
    unsafe { MFCreateAttributes(&mut atributos, 1) }?;
    let atributos = atributos.context("atributos ausentes")?;
    unsafe { atributos.SetUINT32(&MF_SOURCE_READER_DISABLE_DXVA, 1) }?;
    unsafe { MFCreateSourceReaderFromURL(&HSTRING::from(caminho), &atributos) }.context("MP4 invalido")
}

fn imprimir_conteiner(caminho: &str) -> Result<()> {
    let leitor = abrir_leitor(caminho)?;
    for (rotulo, trilha) in [("video", TRILHA_VIDEO), ("audio", TRILHA_AUDIO)] {
        let (mut pacotes, mut chaves, mut bytes) = (0u64, 0u64, 0u64);
        let (mut primeiro, mut ultimo, mut maior_intervalo, mut invertidos) = (None, 0i64, 0i64, 0u64);
        while let Some(amostra) = proxima_amostra(&leitor, trilha)? {
            let pts = unsafe { amostra.GetSampleTime() }?;
            if primeiro.is_some() {
                maior_intervalo = maior_intervalo.max(pts - ultimo);
                invertidos += u64::from(pts < ultimo);
            } else {
                primeiro = Some(pts);
            }
            ultimo = pts;
            pacotes += 1;
            bytes += u64::from(unsafe { amostra.GetTotalLength() }?);
            chaves += u64::from(unsafe { amostra.GetUINT32(&MFSampleExtension_CleanPoint) }.unwrap_or(0) != 0);
        }
        let inicio = primeiro.unwrap_or(0);
        let duracao = (ultimo - inicio) as f64 / UNIDADES_POR_SEGUNDO;
        println!(
            "conteiner {rotulo}: {pacotes} pacotes, {chaves} chave, inicio {:.3}s, duracao {duracao:.2}s, \
             {:.1} pacotes/s, \
             {:.1} Mbps, maior intervalo {:.0} ms, fora de ordem {invertidos}",
            inicio as f64 / UNIDADES_POR_SEGUNDO,
            pacotes.saturating_sub(1) as f64 / duracao.max(0.001),
            bytes as f64 * 8.0 / duracao.max(0.001) / 1e6,
            maior_intervalo as f64 / 10_000.0,
        );
    }
    Ok(())
}

fn nome_fourcc(subtipo: &GUID) -> String {
    subtipo.data1.to_le_bytes().iter().map(|byte| *byte as char).collect()
}

fn pedir_saida(leitor: &IMFSourceReader, trilha: u32, maior: &GUID, subtipo: &GUID) -> Result<()> {
    let tipo = unsafe { MFCreateMediaType() }?;
    unsafe {
        tipo.SetGUID(&MF_MT_MAJOR_TYPE, maior)?;
        tipo.SetGUID(&MF_MT_SUBTYPE, subtipo)?;
        leitor.SetCurrentMediaType(trilha, None, &tipo)?;
    }
    Ok(())
}

fn proxima_amostra(leitor: &IMFSourceReader, trilha: u32) -> Result<Option<IMFSample>> {
    let mut sinalizadores = 0;
    let mut amostra = None;
    unsafe { leitor.ReadSample(trilha, 0, None, Some(&mut sinalizadores), None, Some(&mut amostra)) }?;
    if sinalizadores & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
        return Ok(None);
    }
    if amostra.is_none() {
        bail!("leitor nao entregou amostra");
    }
    Ok(amostra)
}

fn ler_video(leitor: &IMFSourceReader, largura: usize, altura: usize) -> Result<MetricasVideo> {
    let mut metricas = MetricasVideo {
        luma_minima: u8::MAX,
        ..Default::default()
    };
    while let Some(amostra) = proxima_amostra(leitor, TRILHA_VIDEO)? {
        let pts = unsafe { amostra.GetSampleTime() }?;
        if let Some(_primeiro) = metricas.primeiro_pts {
            metricas.maior_intervalo = metricas.maior_intervalo.max(pts - metricas.ultimo_pts);
        } else {
            metricas.primeiro_pts = Some(pts);
        }
        metricas.ultimo_pts = pts;
        metricas.quadros += 1;
        if metricas.quadros % 30 == 1 {
            acumular_luma(&amostra, largura, altura, &mut metricas)?;
        }
    }
    Ok(metricas)
}

fn acumular_luma(amostra: &IMFSample, largura: usize, altura: usize, metricas: &mut MetricasVideo) -> Result<()> {
    let buffer = unsafe { amostra.ConvertToContiguousBuffer() }?;
    let mut ponteiro = std::ptr::null_mut();
    let mut tamanho = 0;
    unsafe { buffer.Lock(&mut ponteiro, None, Some(&mut tamanho)) }?;
    let dados = unsafe { std::slice::from_raw_parts(ponteiro, tamanho as usize) };
    for &luma in dados[..(largura * altura).min(dados.len())].iter().step_by(97) {
        let valor = f64::from(luma);
        metricas.soma_luma += valor;
        metricas.soma_quadrados_luma += valor * valor;
        metricas.amostras_luma += 1;
        metricas.luma_minima = metricas.luma_minima.min(luma);
        metricas.luma_maxima = metricas.luma_maxima.max(luma);
    }
    unsafe { buffer.Unlock() }?;
    Ok(())
}

fn ler_audio(leitor: &IMFSourceReader) -> Result<MetricasAudio> {
    let mut metricas = MetricasAudio::default();
    while let Some(amostra) = proxima_amostra(leitor, TRILHA_AUDIO)? {
        let pts = unsafe { amostra.GetSampleTime() }?;
        metricas.primeiro_pts.get_or_insert(pts);
        metricas.ultimo_pts = pts;
        metricas.blocos += 1;
        let buffer = unsafe { amostra.ConvertToContiguousBuffer() }?;
        let mut ponteiro = std::ptr::null_mut();
        let mut tamanho = 0;
        unsafe { buffer.Lock(&mut ponteiro, None, Some(&mut tamanho)) }?;
        let dados = unsafe { std::slice::from_raw_parts(ponteiro, tamanho as usize) };
        metricas.bytes += u64::from(tamanho);
        for par in dados.chunks_exact(2) {
            metricas.pico = metricas.pico.max(i32::from(i16::from_le_bytes([par[0], par[1]])).abs());
        }
        unsafe { buffer.Unlock() }?;
    }
    Ok(metricas)
}

fn imprimir_video(metricas: &MetricasVideo, tamanho_arquivo: u64) {
    let inicio = metricas.primeiro_pts.unwrap_or(0);
    let duracao = (metricas.ultimo_pts - inicio) as f64 / UNIDADES_POR_SEGUNDO;
    let media = metricas.soma_luma / metricas.amostras_luma.max(1) as f64;
    let variancia = metricas.soma_quadrados_luma / metricas.amostras_luma.max(1) as f64 - media * media;
    println!(
        "arquivo: {:.1} MiB, {:.1} Mbps",
        mib(tamanho_arquivo),
        tamanho_arquivo as f64 * 8.0 / duracao.max(0.001) / 1e6
    );
    println!(
        "quadros: {} decodificados, inicio {:.3}s, duracao {:.2}s, {:.1} fps, maior intervalo {:.0} ms",
        metricas.quadros,
        inicio as f64 / UNIDADES_POR_SEGUNDO,
        duracao,
        metricas.quadros.saturating_sub(1) as f64 / duracao.max(0.001),
        metricas.maior_intervalo as f64 / 10_000.0
    );
    println!(
        "luma: media {media:.1}, desvio {:.1}, min {}, max {}",
        variancia.max(0.0).sqrt(),
        metricas.luma_minima,
        metricas.luma_maxima
    );
}

fn imprimir_audio(audio: &MetricasAudio, video: &MetricasVideo) {
    let inicio = audio.primeiro_pts.unwrap_or(0);
    let duracao = (audio.ultimo_pts - inicio) as f64 / UNIDADES_POR_SEGUNDO;
    let segundos_pcm = audio.bytes as f64 / (48_000.0 * 4.0);
    let fim_video = video.ultimo_pts as f64 / UNIDADES_POR_SEGUNDO;
    println!(
        "audio: {} blocos, inicio {:.3}s, duracao {:.2}s, {:.2}s de PCM, pico {}, \
         diferenca final para o video {:.0} ms",
        audio.blocos,
        inicio as f64 / UNIDADES_POR_SEGUNDO,
        duracao,
        segundos_pcm,
        audio.pico,
        (audio.ultimo_pts as f64 / UNIDADES_POR_SEGUNDO - fim_video) * 1000.0
    );
}

fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}
