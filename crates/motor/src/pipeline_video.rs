use crate::armazenamento::{AnelCompartilhado, ArquivoCircular};
use crate::captura::{Captura, ResultadoEspera};
use crate::codificador_video::{CodificadorVideo, EventoCodificador, PacoteVideo, ParametrosVideo};
use crate::conversor::ConversorNv12;
use crate::dispositivo::DispositivoGrafico;
use crate::tempo::agora;
use crate::tipos_midia::DescritorVideo;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use zaga_nucleo::UNIDADES_POR_SEGUNDO;
use zaga_nucleo::anel::{AnelPacotes, Fluxo};
use zaga_nucleo::cabecalho::extrair_cabecalho;
use zaga_nucleo::config::{Codec, Config};

const ESPERA_POR_QUADRO: Duration = Duration::from_millis(100);
const INTERVALO_QUADRO_CHAVE: i64 = 2 * UNIDADES_POR_SEGUNDO;

pub struct InicioVideo {
    pub anel: AnelCompartilhado,
    pub descritor: Arc<OnceLock<DescritorVideo>>,
    pub largura: u32,
    pub altura: u32,
    pub bitrate: u32,
    pub encoder: String,
}

/// Executa captura, conversão e codificação na mesma thread, guiadas pelos eventos do encoder.
pub fn executar(
    config: Config,
    caminho_anel: PathBuf,
    parar: Arc<AtomicBool>,
    avisar_inicio: SyncSender<Result<InicioVideo>>,
) -> Result<()> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
    let preparado = Pipeline::preparar(&config, &caminho_anel);
    let mut pipeline = match preparado {
        Ok(pipeline) => pipeline,
        Err(erro) => {
            let mensagem = format!("{erro:#}");
            let _ = avisar_inicio.send(Err(erro));
            anyhow::bail!(mensagem);
        }
    };
    let _ = avisar_inicio.send(Ok(pipeline.resumo_inicio()));
    pipeline.rodar(&parar)
}

struct Pipeline {
    grafico: DispositivoGrafico,
    captura: Captura,
    conversor: ConversorNv12,
    codificador: CodificadorVideo,
    anel: AnelCompartilhado,
    descritor: Arc<OnceLock<DescritorVideo>>,
    codec: Codec,
    tamanho_saida: (u32, u32),
    fps: u32,
    bitrate: u32,
    duracao_quadro: i64,
    ultimo_quadro_chave: Option<i64>,
    ultima_textura: Option<ID3D11Texture2D>,
}

impl Pipeline {
    fn preparar(config: &Config, caminho_anel: &std::path::Path) -> Result<Self> {
        let grafico = DispositivoGrafico::criar_no_monitor_principal()?;
        let captura = Captura::iniciar(&grafico, config.fps, config.capturar_cursor)?;
        let entrada = captura.tamanho();
        let tamanho_saida = config.resolucao.aplicar(entrada.0, entrada.1);
        let conversor = ConversorNv12::new(&grafico, entrada, tamanho_saida)?;

        let bitrate = config.bitrate_video(tamanho_saida.0, tamanho_saida.1);
        let parametros = ParametrosVideo {
            codec: config.codec,
            largura: tamanho_saida.0,
            altura: tamanho_saida.1,
            fps: config.fps,
            bitrate,
        };
        let codificador = CodificadorVideo::new(&grafico, &parametros)?;

        let armazenamento = ArquivoCircular::criar(caminho_anel, config.capacidade_anel(bitrate))
            .context("falha ao reservar o buffer em disco")?;
        let anel = Arc::new(Mutex::new(AnelPacotes::new(armazenamento, config.retencao_unidades())));

        Ok(Self {
            grafico,
            captura,
            conversor,
            codificador,
            anel,
            descritor: Arc::new(OnceLock::new()),
            codec: config.codec,
            tamanho_saida,
            fps: config.fps,
            bitrate,
            duracao_quadro: UNIDADES_POR_SEGUNDO / i64::from(config.fps),
            ultimo_quadro_chave: None,
            ultima_textura: None,
        })
    }

    fn resumo_inicio(&self) -> InicioVideo {
        InicioVideo {
            anel: self.anel.clone(),
            descritor: self.descritor.clone(),
            largura: self.tamanho_saida.0,
            altura: self.tamanho_saida.1,
            bitrate: self.bitrate,
            encoder: self.codificador.nome().to_owned(),
        }
    }

    fn rodar(&mut self, parar: &AtomicBool) -> Result<()> {
        while !parar.load(Ordering::Relaxed) {
            match self.codificador.proximo_evento()? {
                EventoCodificador::PrecisaEntrada => {
                    if !self.enviar_proximo_quadro(parar)? {
                        return Ok(());
                    }
                }
                EventoCodificador::TemSaida => self.receber_pacote()?,
                EventoCodificador::Outro => {}
            }
        }
        Ok(())
    }

    fn enviar_proximo_quadro(&mut self, parar: &AtomicBool) -> Result<bool> {
        while !parar.load(Ordering::Relaxed) {
            let (textura, pts) = match self.captura.aguardar_quadro(ESPERA_POR_QUADRO)? {
                ResultadoEspera::Quadro(quadro) => {
                    let entrada = (quadro.largura, quadro.altura);
                    if entrada != self.conversor.entrada() {
                        self.conversor = ConversorNv12::new(&self.grafico, entrada, self.tamanho_saida)?;
                    }
                    (self.conversor.converter(&quadro.textura)?, quadro.pts)
                }
                ResultadoEspera::Descartado => continue,
                ResultadoEspera::TelaParada => {
                    let Some(ultima) = self.ultima_textura.clone() else {
                        continue;
                    };
                    (ultima, agora())
                }
            };
            unsafe { self.grafico.contexto.Flush() };
            self.solicitar_quadro_chave_periodico(pts);
            self.codificador.enviar(&textura, pts, self.duracao_quadro)?;
            self.ultima_textura = Some(textura);
            return Ok(true);
        }
        Ok(false)
    }

    fn solicitar_quadro_chave_periodico(&mut self, pts: i64) {
        let Some(ultimo) = self.ultimo_quadro_chave else {
            self.ultimo_quadro_chave = Some(pts);
            return;
        };
        if pts - ultimo >= INTERVALO_QUADRO_CHAVE {
            self.codificador.forcar_quadro_chave();
            self.ultimo_quadro_chave = Some(pts);
        }
    }

    fn receber_pacote(&self) -> Result<()> {
        self.codificador.receber(|pacote| {
            if pacote.chave {
                self.publicar_descritor(&pacote);
            }
            let mut anel = self.anel.lock().map_err(|_| anyhow::anyhow!("anel envenenado"))?;
            anel.inserir(Fluxo::Video, pacote.pts, pacote.chave, pacote.dados)?;
            Ok(())
        })
    }

    fn publicar_descritor(&self, pacote: &PacoteVideo<'_>) {
        if self.descritor.get().is_some() {
            return;
        }
        let cabecalho = extrair_cabecalho(self.codec, pacote.dados).or_else(|| self.codificador.cabecalho_do_tipo());
        let Some(cabecalho) = cabecalho else {
            return;
        };
        let _ = self.descritor.set(DescritorVideo {
            codec: self.codec,
            largura: self.tamanho_saida.0,
            altura: self.tamanho_saida.1,
            fps: self.fps,
            bitrate: self.bitrate,
            cabecalho,
        });
    }
}
