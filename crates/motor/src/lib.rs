mod armazenamento;
mod captura;
mod codificador_audio;
mod codificador_video;
mod controlador;
mod conversor;
mod dispositivo;
mod gravador_clipe;
mod inicializacao;
mod ipc;
mod janela;
mod origem;
mod pipeline_audio;
mod pipeline_video;
mod registro;
mod sessao;
mod tempo;
mod tipos_midia;
mod uso_cpu;

use anyhow::{Context, Result};
use controlador::Controlador;
use registro::registrar;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Media::MediaFoundation::{MF_VERSION, MFShutdown, MFStartup};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Shell::{FOLDERID_Videos, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
use zaga_nucleo::protocolo::{Pedido, Resposta};

static CONTROLADOR: OnceLock<Arc<Controlador>> = OnceLock::new();
static ABRIR: AtomicBool = AtomicBool::new(false);
static ENCERRADO: AtomicBool = AtomicBool::new(false);

const VARIAVEL_PASTA_DADOS: &str = "ZAGA_DADOS";

/// Inicia captura e bandeja em threads do próprio aplicativo.
pub fn iniciar() -> Result<()> {
    let (envio, recebimento) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new().name("zaga_motor".into()).spawn(move || {
        if let Err(erro) = executar(envio) {
            registrar(format!("motor encerrado com erro: {erro:#}"));
        }
        ENCERRADO.store(true, Ordering::Release);
    })?;
    recebimento
        .recv()
        .context("motor encerrou durante a inicialização")?
        .map_err(anyhow::Error::msg)
}

/// Executa comandos internos sem criar processos ou serializar mensagens.
pub fn enviar(pedido: Pedido) -> Result<Resposta> {
    let controlador = CONTROLADOR.get().context("motor ainda não iniciado")?;
    if pedido == Pedido::Encerrar {
        janela::encerrar();
    }
    Ok(ipc::responder(controlador, pedido))
}

/// Consome o pedido da bandeja para reabrir a interface.
pub fn consumir_abertura() -> bool {
    ABRIR.swap(false, Ordering::AcqRel)
}

/// Informa se a captura concluiu o encerramento solicitado na bandeja.
pub fn encerrado() -> bool {
    ENCERRADO.load(Ordering::Acquire)
}

fn executar(pronto: std::sync::mpsc::SyncSender<Result<(), String>>) -> Result<()> {
    let pasta_dados = pasta_dados()?;
    std::fs::create_dir_all(&pasta_dados).context("falha ao criar a pasta de dados")?;
    registro::iniciar(&pasta_dados);

    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
    unsafe { MFStartup(MF_VERSION, 0) }.context("Media Foundation indisponivel")?;

    let controlador = Arc::new(Controlador::new(pasta_dados.clone(), pasta_clipes(&pasta_dados)?));
    controlador.iniciar_captura();
    CONTROLADOR
        .set(controlador.clone())
        .map_err(|_| anyhow::anyhow!("motor já iniciado"))?;

    let controlador_ipc = controlador.clone();
    let thread_principal = unsafe { GetCurrentThreadId() };
    std::thread::Builder::new()
        .name("zaga_ipc".into())
        .spawn(move || ipc::atender(controlador_ipc, thread_principal))?;

    let resultado = janela::executar(controlador.clone(), pronto);
    controlador.encerrar_captura();
    let _ = unsafe { MFShutdown() };
    resultado
}

fn pasta_dados() -> Result<PathBuf> {
    if let Some(pasta) = std::env::var_os(VARIAVEL_PASTA_DADOS) {
        return Ok(PathBuf::from(pasta));
    }
    let local = std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA ausente")?;
    Ok(PathBuf::from(local).join("Zaga"))
}

fn pasta_clipes(pasta_dados: &std::path::Path) -> Result<PathBuf> {
    if std::env::var_os(VARIAVEL_PASTA_DADOS).is_some() {
        return Ok(pasta_dados.join("clipes"));
    }
    let videos = unsafe { SHGetKnownFolderPath(&FOLDERID_Videos, KF_FLAG_DEFAULT, None) }?;
    let texto = unsafe { videos.to_string() }?;
    unsafe { windows::Win32::System::Com::CoTaskMemFree(Some(videos.0.cast())) };
    Ok(PathBuf::from(texto).join("Zaga"))
}
