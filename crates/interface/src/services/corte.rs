use crate::services::midia::{abrir_leitor, posicionar_leitor};
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFSinkWriter, IMFSourceReader, MF_ACCESSMODE_READWRITE, MF_FILEFLAGS_NONE,
    MF_OPENMODE_DELETE_IF_EXIST, MF_SINK_WRITER_DISABLE_THROTTLING, MF_SOURCE_READER_ANY_STREAM,
    MF_SOURCE_READERF_ENDOFSTREAM, MF_TRANSCODE_CONTAINERTYPE, MFCreateAttributes, MFCreateFile,
    MFCreateSinkWriterFromURL, MFMediaType_Audio, MFMediaType_Video, MFTranscodeContainerType_MPEG4,
};
use windows::core::{HSTRING, PCWSTR};

const TRILHAS_MAXIMAS: u32 = 8;

/// Copia um trecho do clipe para um novo MP4 sem recodificar, preservando a qualidade original.
///
/// O início recua até o quadro chave anterior, pois sem recodificar o vídeo só pode começar em um deles.
pub fn salvar_trecho(origem: &Path, inicio: i64, fim: i64) -> Result<PathBuf> {
    assert!(fim > inicio, "trecho precisa ter duração positiva");
    let destino = caminho_do_corte(origem);
    let temporario = destino.with_extension("parcial.mp4");
    if let Err(erro) = copiar_trecho(origem, &temporario, inicio, fim) {
        let _ = std::fs::remove_file(&temporario);
        return Err(erro);
    }
    std::fs::rename(&temporario, &destino).context("falha ao concluir o clipe cortado")?;
    Ok(destino)
}

fn carimbos_em_ordem(anterior: Option<i64>, atual: i64) -> bool {
    anterior.is_none_or(|anterior| atual > anterior)
}

fn caminho_do_corte(origem: &Path) -> PathBuf {
    let pasta = origem.parent().map_or_else(PathBuf::new, Path::to_path_buf);
    let nome = origem.file_stem().and_then(|nome| nome.to_str()).unwrap_or("Clipe");
    let primeiro = pasta.join(format!("{nome} cortado.mp4"));
    if !primeiro.exists() {
        return primeiro;
    }
    (2..)
        .map(|numero| pasta.join(format!("{nome} cortado {numero}.mp4")))
        .find(|caminho| !caminho.exists())
        .unwrap_or(primeiro)
}

fn copiar_trecho(origem: &Path, destino: &Path, inicio: i64, fim: i64) -> Result<()> {
    let leitor = abrir_leitor(origem, false)?;
    let escritor = criar_escritor(destino)?;
    let (mapa, trilha_video) = mapear_trilhas(&leitor, &escritor)?;
    posicionar_leitor(&leitor, inicio)?;
    unsafe { escritor.BeginWriting() }.context("falha ao iniciar o novo clipe")?;
    copiar_amostras(&leitor, &escritor, &mapa, trilha_video, fim)?;
    unsafe { escritor.Finalize() }.context("falha ao finalizar o novo clipe")?;
    Ok(())
}

fn criar_escritor(destino: &Path) -> Result<IMFSinkWriter> {
    let mut atributos: Option<IMFAttributes> = None;
    unsafe { MFCreateAttributes(&mut atributos, 2) }?;
    let atributos = atributos.context("atributos ausentes")?;
    unsafe {
        atributos.SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4)?;
        atributos.SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)?;
    }
    let url = HSTRING::from(destino.as_os_str());
    let arquivo = unsafe {
        MFCreateFile(
            MF_ACCESSMODE_READWRITE,
            MF_OPENMODE_DELETE_IF_EXIST,
            MF_FILEFLAGS_NONE,
            &url,
        )
    }
    .context("falha ao criar o novo clipe")?;
    unsafe { MFCreateSinkWriterFromURL(PCWSTR::null(), &arquivo, &atributos) }.context("falha ao abrir o escritor")
}

fn mapear_trilhas(leitor: &IMFSourceReader, escritor: &IMFSinkWriter) -> Result<(Vec<Option<u32>>, u32)> {
    let mut mapa = Vec::new();
    let mut trilha_video = None;
    for indice in 0..TRILHAS_MAXIMAS {
        let Ok(tipo) = (unsafe { leitor.GetNativeMediaType(indice, 0) }) else {
            break;
        };
        let maior = unsafe { tipo.GetMajorType() }?;
        if maior != MFMediaType_Video && maior != MFMediaType_Audio {
            mapa.push(None);
            continue;
        }
        unsafe { leitor.SetStreamSelection(indice, true) }?;
        let saida = unsafe { escritor.AddStream(&tipo) }?;
        unsafe { escritor.SetInputMediaType(saida, &tipo, None) }?;
        mapa.push(Some(saida));
        if maior == MFMediaType_Video {
            trilha_video = Some(indice);
        }
    }
    Ok((mapa, trilha_video.context("clipe sem trilha de video")?))
}

fn copiar_amostras(
    leitor: &IMFSourceReader,
    escritor: &IMFSinkWriter,
    mapa: &[Option<u32>],
    trilha_video: u32,
    fim: i64,
) -> Result<()> {
    let mut base = None;
    let mut amostras_de_video = 0u64;
    let mut ultimo_pts_video = None;
    loop {
        let mut trilha = 0;
        let mut sinalizadores = 0;
        let mut amostra = None;
        unsafe {
            leitor.ReadSample(
                MF_SOURCE_READER_ANY_STREAM.0 as u32,
                0,
                Some(&mut trilha),
                Some(&mut sinalizadores),
                None,
                Some(&mut amostra),
            )
        }?;
        if sinalizadores & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            break;
        }
        let Some(amostra) = amostra else {
            continue;
        };
        let pts = unsafe { amostra.GetSampleTime() }?;
        if trilha == trilha_video {
            if !carimbos_em_ordem(ultimo_pts_video, pts) {
                bail!("este clipe usa quadros B e nao pode ser cortado sem recodificar");
            }
            ultimo_pts_video = Some(pts);
        }
        if trilha == trilha_video && base.is_none() {
            base = Some(pts);
        }
        let Some(base) = base else {
            continue;
        };
        if pts < base || pts > fim {
            if trilha == trilha_video && pts > fim {
                break;
            }
            continue;
        }
        let Some(saida) = mapa.get(trilha as usize).copied().flatten() else {
            continue;
        };
        unsafe {
            amostra.SetSampleTime(pts - base)?;
            escritor.WriteSample(saida, &amostra)?;
        }
        amostras_de_video += u64::from(trilha == trilha_video);
    }
    if amostras_de_video == 0 {
        bail!("trecho sem quadros de video");
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn quadros_b_sao_detectados_pela_inversao_de_carimbos() {
        assert!(carimbos_em_ordem(None, 0));
        assert!(carimbos_em_ordem(Some(10), 20));
        assert!(!carimbos_em_ordem(Some(30), 20));
    }
}
