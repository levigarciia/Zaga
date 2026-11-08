use crate::armazenamento::AnelCompartilhado;
use crate::tempo::carimbo_para_nome_de_arquivo;
use crate::tipos_midia::{
    DescritorAudio, DescritorVideo, QUADROS_POR_PACOTE_AAC, TAXA_AUDIO, tipo_audio_do_descritor,
    tipo_video_do_descritor,
};
use anyhow::{Context, Result, anyhow, bail};
use std::path::{Path, PathBuf};
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFSample, IMFSinkWriter, MF_ACCESSMODE_READWRITE, MF_FILEFLAGS_NONE, MF_OPENMODE_DELETE_IF_EXIST,
    MF_SINK_WRITER_DISABLE_THROTTLING, MF_TRANSCODE_CONTAINERTYPE, MFCreateAttributes, MFCreateFile,
    MFCreateMemoryBuffer, MFCreateSample, MFCreateSinkWriterFromURL, MFSampleExtension_CleanPoint,
    MFTranscodeContainerType_MPEG4,
};
use windows::core::{HSTRING, PCWSTR};
use zaga_nucleo::UNIDADES_POR_SEGUNDO;
use zaga_nucleo::anel::{Fluxo, MetaPacote, SelecaoClipe};

const DURACAO_PACOTE_AAC: i64 = QUADROS_POR_PACOTE_AAC as i64 * UNIDADES_POR_SEGUNDO / TAXA_AUDIO as i64;
static EXPORTACAO: Mutex<()> = Mutex::new(());
static PROXIMO_ARQUIVO: AtomicU64 = AtomicU64::new(0);

/// Copia o trecho final do anel para um MP4 sem recodificar, deixando a captura seguir durante a gravação.
pub fn salvar_clipe(
    anel: &AnelCompartilhado,
    video: &DescritorVideo,
    audio: Option<&DescritorAudio>,
    segundos: u32,
    origem: &str,
    pasta: &Path,
) -> Result<PathBuf> {
    assert!(segundos > 0, "duração do clipe precisa ser positiva");
    let duracao = i64::from(segundos) * UNIDADES_POR_SEGUNDO;
    let selecao = {
        let anel = anel.lock().map_err(|_| anyhow!("anel envenenado"))?;
        anel.selecionar_clipe(duracao)
            .context("ainda nao ha video suficiente no buffer")?
    };

    salvar_selecao(anel, video, audio, &selecao, origem, pasta)
}

/// Exporta um destaque por intervalo exato e persiste seu título e origem junto ao MP4.
pub fn salvar_destaque(
    anel: &AnelCompartilhado,
    video: &DescritorVideo,
    audio: Option<&DescritorAudio>,
    destaque: &zaga_nucleo::autoclipagem::Destaque,
    pasta: &Path,
) -> Result<PathBuf> {
    let selecao = anel
        .lock()
        .map_err(|_| anyhow!("anel indisponível"))?
        .selecionar_intervalo(
            crate::tempo::de_unix_ms(destaque.inicio_ms),
            crate::tempo::de_unix_ms(destaque.fim_ms),
        )
        .context("O trecho do destaque não está completo no buffer. Aumente a retenção ou o limite de espaço")?;
    let destino = salvar_selecao(anel, video, audio, &selecao, &destaque.nome_arquivo(), pasta)?;
    let metadados = destino.with_extension("json");
    let temporario = metadados.with_extension("json.tmp");
    std::fs::write(&temporario, serde_json::to_vec_pretty(destaque)?)?;
    std::fs::rename(temporario, metadados)?;
    Ok(destino)
}

fn salvar_selecao(
    anel: &AnelCompartilhado,
    video: &DescritorVideo,
    audio: Option<&DescritorAudio>,
    selecao: &SelecaoClipe,
    origem: &str,
    pasta: &Path,
) -> Result<PathBuf> {
    let _exportacao = EXPORTACAO.lock().map_err(|_| anyhow!("Exportação indisponível"))?;
    std::fs::create_dir_all(pasta).context("falha ao criar a pasta de clipes")?;
    let destino = caminho_livre(pasta, origem);
    let temporario = destino.with_extension("parcial.mp4");
    let resultado = gravar_mp4(anel, video, audio, selecao, &temporario);
    if let Err(erro) = resultado {
        let _ = std::fs::remove_file(&temporario);
        return Err(erro);
    }
    std::fs::rename(&temporario, &destino).context("falha ao concluir o arquivo do clipe")?;
    Ok(destino)
}

fn caminho_livre(pasta: &Path, origem: &str) -> PathBuf {
    let numero = PROXIMO_ARQUIVO.fetch_add(1, Ordering::Relaxed);
    let base = format!(
        "{origem} {} {} {numero}",
        carimbo_para_nome_de_arquivo(),
        std::process::id()
    );
    let candidato = pasta.join(format!("{base}.mp4"));
    if !candidato.exists() {
        return candidato;
    }
    (2..)
        .map(|numero| pasta.join(format!("{base} {numero}.mp4")))
        .find(|caminho| !caminho.exists())
        .unwrap_or(candidato)
}

fn gravar_mp4(
    anel: &AnelCompartilhado,
    video: &DescritorVideo,
    audio: Option<&DescritorAudio>,
    selecao: &SelecaoClipe,
    caminho: &Path,
) -> Result<()> {
    let escritor = criar_escritor(caminho)?;
    let trilha_video = adicionar_trilha(&escritor, &tipo_video_do_descritor(video)?)?;
    let trilha_audio = match audio {
        Some(descritor) => Some(adicionar_trilha(&escritor, &tipo_audio_do_descritor(descritor)?)?),
        None => None,
    };
    unsafe { escritor.BeginWriting() }.context("falha ao iniciar o MP4")?;

    let duracoes_video = duracoes_de_video(&selecao.pacotes, UNIDADES_POR_SEGUNDO / i64::from(video.fps));
    let mut proxima_duracao_video = duracoes_video.into_iter();
    let mut buffer = Vec::new();
    for meta in &selecao.pacotes {
        let (trilha, duracao) = match (meta.fluxo, trilha_audio) {
            (Fluxo::Video, _) => (trilha_video, proxima_duracao_video.next().context("duracao ausente")?),
            (Fluxo::Audio, Some(trilha)) => (trilha, DURACAO_PACOTE_AAC),
            (Fluxo::Audio, None) => continue,
        };
        ler_pacote(anel, meta, &mut buffer)?;
        let amostra = criar_amostra(&buffer, meta.pts - selecao.inicio, duracao, meta.chave)?;
        unsafe { escritor.WriteSample(trilha, &amostra) }.context("falha ao gravar amostra")?;
    }
    if selecao.pacotes.is_empty() {
        bail!("selecao de clipe vazia");
    }
    unsafe { escritor.Finalize() }.context("falha ao finalizar o MP4")?;
    Ok(())
}

fn criar_escritor(caminho: &Path) -> Result<IMFSinkWriter> {
    let mut atributos: Option<IMFAttributes> = None;
    unsafe { MFCreateAttributes(&mut atributos, 3) }?;
    let atributos = atributos.context("atributos ausentes")?;
    unsafe {
        atributos.SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4)?;
        atributos.SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)?;
    }
    let url = HSTRING::from(caminho.as_os_str());
    let arquivo = unsafe {
        MFCreateFile(
            MF_ACCESSMODE_READWRITE,
            MF_OPENMODE_DELETE_IF_EXIST,
            MF_FILEFLAGS_NONE,
            &url,
        )
    }
    .context("falha ao criar o arquivo do clipe")?;
    unsafe { MFCreateSinkWriterFromURL(PCWSTR::null(), &arquivo, &atributos) }
        .context("falha ao abrir o escritor do clipe")
}

fn adicionar_trilha(
    escritor: &IMFSinkWriter,
    tipo: &windows::Win32::Media::MediaFoundation::IMFMediaType,
) -> Result<u32> {
    let trilha = unsafe { escritor.AddStream(tipo) }.context("MP4 recusou a trilha")?;
    unsafe { escritor.SetInputMediaType(trilha, tipo, None) }.context("MP4 recusou o tipo de entrada")?;
    Ok(trilha)
}

fn duracoes_de_video(pacotes: &[MetaPacote], duracao_padrao: i64) -> Vec<i64> {
    let tempos: Vec<i64> = pacotes
        .iter()
        .filter(|meta| meta.fluxo == Fluxo::Video)
        .map(|meta| meta.pts)
        .collect();
    let mut duracoes: Vec<i64> = tempos.windows(2).map(|par| (par[1] - par[0]).max(1)).collect();
    if !tempos.is_empty() {
        duracoes.push(duracao_padrao);
    }
    duracoes
}

fn ler_pacote(anel: &AnelCompartilhado, meta: &MetaPacote, buffer: &mut Vec<u8>) -> Result<()> {
    buffer.resize(meta.tamanho as usize, 0);
    let mut anel = anel.lock().map_err(|_| anyhow!("anel envenenado"))?;
    anel.ler(meta, buffer)?;
    Ok(())
}

fn criar_amostra(dados: &[u8], pts: i64, duracao: i64, chave: bool) -> Result<IMFSample> {
    let buffer = unsafe { MFCreateMemoryBuffer(dados.len() as u32) }?;
    let mut ponteiro = std::ptr::null_mut();
    unsafe {
        buffer.Lock(&mut ponteiro, None, None)?;
        std::ptr::copy_nonoverlapping(dados.as_ptr(), ponteiro, dados.len());
        buffer.Unlock()?;
        buffer.SetCurrentLength(dados.len() as u32)?;
    }
    let amostra = unsafe { MFCreateSample() }?;
    unsafe {
        amostra.AddBuffer(&buffer)?;
        amostra.SetSampleTime(pts)?;
        amostra.SetSampleDuration(duracao)?;
        if chave {
            amostra.SetUINT32(&MFSampleExtension_CleanPoint, 1)?;
        }
    }
    Ok(amostra)
}

#[cfg(test)]
mod testes {
    use super::*;
    use zaga_nucleo::anel::{AnelPacotes, Armazenamento};

    struct Memoria(Vec<u8>);

    impl Armazenamento for Memoria {
        fn capacidade(&self) -> u64 {
            self.0.len() as u64
        }
        fn escrever(&mut self, _: u64, _: &[u8]) -> std::io::Result<()> {
            Ok(())
        }
        fn ler(&mut self, _: u64, _: &mut [u8]) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn duracao_de_video_segue_os_carimbos_e_ignora_audio() {
        let mut anel = AnelPacotes::new(Memoria(vec![0; 64]), 1000);
        anel.inserir(Fluxo::Video, 10, true, &[0]).unwrap();
        anel.inserir(Fluxo::Audio, 15, false, &[0]).unwrap();
        anel.inserir(Fluxo::Video, 30, false, &[0]).unwrap();
        let selecao = anel.selecionar_clipe(100).unwrap();
        assert_eq!(duracoes_de_video(&selecao.pacotes, 7), vec![20, 7]);
    }
}
