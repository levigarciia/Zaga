use crate::armazenamento::{AnelCompartilhado, ArquivoCircular};
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
    exportacoes: Arc<AtomicU32>,
    limites_anel: Mutex<(u64, i64)>,
    pasta_dados: PathBuf,
}

impl Sessao {
    pub(crate) fn atualizar_retencao(&self, config: &Config) -> Result<()> {
        let limites = (config.capacidade_anel(self.bitrate), config.retencao_unidades());
        let mut atuais = self.limites_anel.lock().map_err(|_| anyhow!("Limites indisponíveis"))?;
        if *atuais == limites || self.exportacoes.load(Ordering::Acquire) != 0 {
            return Ok(());
        }
        let numero = PROXIMA_SESSAO.fetch_add(1, Ordering::Relaxed);
        let caminho = self
            .pasta_dados
            .join(format!("anel_{}_{numero}.bin", std::process::id()));
        let armazenamento = ArquivoCircular::criar(&caminho, limites.0)?;
        self.anel
            .lock()
            .map_err(|_| anyhow!("Anel indisponível"))?
            .migrar(armazenamento, limites.1)?;
        *atuais = limites;
        Ok(())
    }

    /// Exporta um evento em uma thread própria, mantendo o anel vivo até terminar a leitura.
    pub fn salvar_destaque(
        &self,
        destaque: zaga_nucleo::autoclipagem::Destaque,
        pasta: PathBuf,
        concluir: impl FnOnce(Result<PathBuf>) + Send + 'static,
    ) -> Result<()> {
        let video = self
            .descritor_video
            .get()
            .cloned()
            .context("Vídeo ainda indisponível")?;
        let audio = self.descritor_audio.get().cloned();
        let anel = self.anel.clone();
        let pasta_dados = self.pasta_dados.clone();
        let exportacao = Exportacao::iniciar(&self.exportacoes);
        thread::Builder::new().name("zaga_destaque".into()).spawn(move || {
            let _exportacao = exportacao;
            baixar_prioridade_da_thread();
            let _com = iniciar_com();
            concluir(crate::gravador_clipe::salvar_destaque(
                &anel,
                &video,
                audio.as_ref(),
                &destaque,
                &pasta,
                &pasta_dados,
            ));
        })?;
        Ok(())
    }
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
            exportacoes: Arc::new(AtomicU32::new(0)),
            limites_anel: Mutex::new((config.capacidade_anel(inicio.bitrate), config.retencao_unidades())),
            pasta_dados: pasta_dados.to_path_buf(),
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
        let exportacao = Exportacao::iniciar(&self.exportacoes);
        thread::Builder::new().name("zaga_clipe".into()).spawn(move || {
            let _exportacao = exportacao;
            baixar_prioridade_da_thread();
            let _com = iniciar_com();
            concluir(salvar_clipe(&anel, &video, audio.as_ref(), segundos, &origem, &pasta));
        })?;
        Ok(())
    }
}

struct Exportacao(Arc<AtomicU32>);

impl Exportacao {
    fn iniciar(contador: &Arc<AtomicU32>) -> Self {
        contador.fetch_add(1, Ordering::AcqRel);
        Self(contador.clone())
    }
}

impl Drop for Exportacao {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
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
        while self.exportacoes.load(Ordering::Acquire) != 0 {
            thread::sleep(Duration::from_millis(20));
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

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn exportacoes_bloqueiam_migracao_ate_a_ultima_conclusao() {
        use zaga_nucleo::anel::{AnelPacotes, Fluxo};

        let pasta = std::env::temp_dir().join(format!("zaga_migracao_sessao_{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let config = Config::default();
        let limites = (config.capacidade_anel(0), config.retencao_unidades());
        let armazenamento = ArquivoCircular::criar(&pasta.join("original.bin"), limites.0).unwrap();
        let mut anel = AnelPacotes::new(armazenamento, limites.1);
        anel.inserir(Fluxo::Video, 0, true, &[1, 2, 3]).unwrap();
        anel.inserir(Fluxo::Video, UNIDADES_POR_SEGUNDO, false, &[4, 5])
            .unwrap();
        let sessao = Sessao {
            parar: Arc::new(AtomicBool::new(false)),
            video: None,
            audio: None,
            microfone: None,
            anel: Arc::new(Mutex::new(anel)),
            descritor_video: Arc::new(OnceLock::new()),
            descritor_audio: Arc::new(OnceLock::new()),
            largura: 2,
            altura: 2,
            bitrate: 0,
            encoder: String::new(),
            exportacoes: Arc::new(AtomicU32::new(0)),
            limites_anel: Mutex::new(limites),
            pasta_dados: pasta.clone(),
        };
        let nova = Config {
            retencao_segundos: 30,
            ..config
        };
        let manual = Exportacao::iniciar(&sessao.exportacoes);
        let destaque = Exportacao::iniciar(&sessao.exportacoes);
        sessao.atualizar_retencao(&nova).unwrap();
        assert_eq!(*sessao.limites_anel.lock().unwrap(), limites);
        drop(manual);
        sessao.atualizar_retencao(&nova).unwrap();
        assert_eq!(*sessao.limites_anel.lock().unwrap(), limites);
        drop(destaque);
        sessao.atualizar_retencao(&nova).unwrap();
        assert_eq!(
            *sessao.limites_anel.lock().unwrap(),
            (nova.capacidade_anel(0), nova.retencao_unidades())
        );
        let mut anel = sessao.anel.lock().unwrap();
        let selecao = anel.selecionar_clipe(UNIDADES_POR_SEGUNDO).unwrap();
        let mut dados = [0; 3];
        anel.ler(&selecao.pacotes[0], &mut dados).unwrap();
        assert_eq!(dados, [1, 2, 3]);
        drop(anel);
        drop(sessao);
        assert_eq!(std::fs::read_dir(&pasta).unwrap().count(), 0);
        std::fs::remove_dir(pasta).unwrap();
    }
}
