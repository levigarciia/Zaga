use crate::armazenamento::AnelCompartilhado;
use crate::gravador_clipe::salvar_clipe;
use crate::pipeline_audio;
use crate::pipeline_video::{self, InicioVideo};
use crate::registro::registrar;
use crate::tipos_midia::{DescritorAudio, DescritorVideo};
use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use zaga_nucleo::UNIDADES_POR_SEGUNDO;
use zaga_nucleo::config::Config;
use zaga_nucleo::mistura::FilaMicrofone;
use zaga_nucleo::protocolo::ResumoSessao;

const ESPERA_MAXIMA_ENCERRAMENTO: Duration = Duration::from_secs(2);

static PROXIMA_SESSAO: AtomicU32 = AtomicU32::new(0);

/// Conjunto de threads de captura em andamento. Encerrar a sessão libera a GPU e apaga o buffer em disco.
pub struct Sessao {
    parar: Arc<AtomicBool>,
    video: Option<JoinHandle<()>>,
    audio: Option<JoinHandle<()>>,
    microfone: Option<JoinHandle<()>>,
    anel: AnelCompartilhado,
    descritor_video: Arc<OnceLock<DescritorVideo>>,
    descritor_audio: Arc<OnceLock<DescritorAudio>>,
    largura: u32,
    altura: u32,
    bitrate: u32,
    encoder: String,
}

impl Sessao {
    pub fn iniciar(config: &Config, pasta_dados: &Path, ultimo_erro: Arc<Mutex<Option<String>>>) -> Result<Self> {
        let parar = Arc::new(AtomicBool::new(false));
        let (avisar_inicio, inicio_recebido) = sync_channel(1);
        let numero = PROXIMA_SESSAO.fetch_add(1, Ordering::Relaxed);
        let caminho_anel = pasta_dados.join(format!("anel_{}_{numero}.bin", std::process::id()));
        let config_video = config.clone();
        let parar_video = parar.clone();
        let video = thread::Builder::new().name("zaga_video".into()).spawn(move || {
            if let Err(erro) = pipeline_video::executar(config_video, caminho_anel, parar_video, avisar_inicio) {
                registrar(format!("pipeline de video encerrado: {erro:#}"));
            }
        })?;

        let inicio: InicioVideo = inicio_recebido
            .recv()
            .context("pipeline de video encerrou antes de iniciar")?
            .context("falha ao iniciar a captura")?;

        let descritor_audio = Arc::new(OnceLock::new());
        let fila_microfone = config
            .capturar_microfone
            .then(|| Arc::new(Mutex::new(FilaMicrofone::default())));
        let microfone = match &fila_microfone {
            Some(fila) => {
                let fila = fila.clone();
                let parar_microfone = parar.clone();
                let trabalho = move || pipeline_audio::executar_microfone(fila, parar_microfone, ultimo_erro);
                Some(thread::Builder::new().name("zaga_microfone".into()).spawn(trabalho)?)
            }
            None => None,
        };
        let audio = if config.capturar_audio || config.capturar_microfone {
            let anel = inicio.anel.clone();
            let descritor = descritor_audio.clone();
            let parar_audio = parar.clone();
            let incluir_sistema = config.capturar_audio;
            let trabalho =
                move || pipeline_audio::executar(anel, descritor, parar_audio, incluir_sistema, fila_microfone);
            Some(thread::Builder::new().name("zaga_audio".into()).spawn(trabalho)?)
        } else {
            None
        };

        Ok(Self {
            parar,
            video: Some(video),
            audio,
            microfone,
            anel: inicio.anel,
            descritor_video: inicio.descritor,
            descritor_audio,
            largura: inicio.largura,
            altura: inicio.altura,
            bitrate: inicio.bitrate,
            encoder: inicio.encoder,
        })
    }

    pub fn resumo(&self) -> ResumoSessao {
        let duracao = self.anel.lock().map_or(0, |anel| anel.duracao_disponivel());
        ResumoSessao {
            largura: self.largura,
            altura: self.altura,
            bitrate_video: self.bitrate,
            encoder: self.encoder.clone(),
            segundos_disponiveis: (duracao / UNIDADES_POR_SEGUNDO) as u32,
        }
    }

    pub fn trabalhando(&self) -> bool {
        self.video.as_ref().is_some_and(|video| !video.is_finished())
    }

    /// Dispara a gravação de um clipe em segundo plano e devolve o resultado por `concluir`.
    pub fn salvar_em_segundo_plano(
        &self,
        segundos: u32,
        origem: String,
        pasta: PathBuf,
        concluir: impl FnOnce(Result<PathBuf>) + Send + 'static,
    ) -> Result<()> {
        let video = self
            .descritor_video
            .get()
            .cloned()
            .ok_or_else(|| anyhow!("video ainda nao disponivel"))?;
        let audio = self.descritor_audio.get().cloned();
        let anel = self.anel.clone();
        thread::Builder::new().name("zaga_clipe".into()).spawn(move || {
            baixar_prioridade_da_thread();
            let _com = iniciar_com();
            concluir(salvar_clipe(&anel, &video, audio.as_ref(), segundos, &origem, &pasta));
        })?;
        Ok(())
    }
}

impl Drop for Sessao {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::Relaxed);
        let limite = Instant::now() + ESPERA_MAXIMA_ENCERRAMENTO;
        for trabalho in [self.video.take(), self.audio.take(), self.microfone.take()]
            .into_iter()
            .flatten()
        {
            while !trabalho.is_finished() && Instant::now() < limite {
                thread::sleep(Duration::from_millis(10));
            }
            if trabalho.is_finished() {
                let _ = trabalho.join();
            } else {
                registrar("thread de captura nao encerrou a tempo");
            }
        }
    }
}

fn baixar_prioridade_da_thread() {
    use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL};
    let _ = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL) };
}

fn iniciar_com() -> windows::core::HRESULT {
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
}
