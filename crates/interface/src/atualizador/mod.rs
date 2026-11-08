mod aplicador;
mod github;

use anyhow::{Context, Result};
use aplicador::Plano;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zaga_nucleo::protocolo::{Pedido, Resposta};

const VERSAO: &str = env!("CARGO_PKG_VERSION");
const INTERVALO: Duration = Duration::from_secs(6 * 60 * 60);
static ESTADO: OnceLock<Mutex<Estado>> = OnceLock::new();
static VERIFICANDO: AtomicBool = AtomicBool::new(false);
static REINICIAR: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) enum Situacao {
    #[default]
    Aguardando,
    Verificando,
    Atualizado,
    Baixando(String),
    Pronta(String),
    Falha(String),
}

#[derive(Default)]
struct Estado {
    situacao: Situacao,
    plano: Option<Plano>,
}

fn estado() -> &'static Mutex<Estado> {
    ESTADO.get_or_init(|| Mutex::new(Estado::default()))
}

pub(crate) fn situacao() -> Situacao {
    estado()
        .lock()
        .map(|estado| estado.situacao.clone())
        .unwrap_or_default()
}

pub(crate) fn isolado() -> bool {
    ["ZAGA_DADOS", "ZAGA_PIPE", "ZAGA_TESTE_SEM_FOCO"]
        .iter()
        .any(|nome| std::env::var_os(nome).is_some())
}

fn habilitado() -> bool {
    matches!(zaga_motor::enviar(Pedido::ObterEstado), Ok(Resposta::Estado(estado))
        if estado.config.atualizar_automaticamente)
}

pub(crate) fn iniciar() {
    if cfg!(debug_assertions) || isolado() {
        return;
    }
    let _ = std::thread::Builder::new().name("zaga_atualizacoes".into()).spawn(|| {
        limpar_cache();
        std::thread::sleep(Duration::from_secs(20));
        let mut proxima = Instant::now();
        while !zaga_motor::encerrado() {
            if habilitado() && Instant::now() >= proxima {
                verificar();
                proxima = Instant::now() + INTERVALO;
            }
            std::thread::sleep(Duration::from_secs(10));
        }
    });
}

pub(crate) fn verificar() {
    if isolado() || VERIFICANDO.swap(true, Ordering::AcqRel) {
        return;
    }
    if matches!(situacao(), Situacao::Pronta(_)) {
        VERIFICANDO.store(false, Ordering::Release);
        return;
    }
    definir_situacao(Situacao::Verificando);
    if let Err(erro) = std::thread::Builder::new().name("zaga_download".into()).spawn(|| {
        if let Err(erro) = preparar() {
            registrar(&format!("Falha na atualização: {erro:#}"));
            definir_situacao(Situacao::Falha(
                "Não foi possível atualizar. Tente verificar novamente.".into(),
            ));
        }
        VERIFICANDO.store(false, Ordering::Release);
    }) {
        definir_situacao(Situacao::Falha(format!(
            "Não foi possível iniciar a verificação: {erro}"
        )));
        VERIFICANDO.store(false, Ordering::Release);
    }
}

fn preparar() -> Result<()> {
    let Some(atualizacao) = github::consultar(VERSAO)? else {
        definir_situacao(Situacao::Atualizado);
        return Ok(());
    };
    definir_situacao(Situacao::Baixando(atualizacao.versao.clone()));
    let numero = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let pasta = pasta_cache()?.join(format!("sessao_{}_{numero}", std::process::id()));
    std::fs::create_dir_all(&pasta)?;
    let arquivo = pasta.join("nova.exe");
    github::baixar(&atualizacao, &arquivo)?;
    let plano = Plano {
        alvo: std::env::current_exe()?.canonicalize()?,
        arquivo,
        tamanho: atualizacao.tamanho,
        sha256: atualizacao.sha256,
        pid: std::process::id(),
        reiniciar: false,
    };
    let mut estado = estado()
        .lock()
        .map_err(|_| anyhow::anyhow!("Estado da atualização indisponível"))?;
    estado.plano = Some(plano);
    estado.situacao = Situacao::Pronta(atualizacao.versao);
    Ok(())
}

pub(crate) fn solicitar_reinicio() -> bool {
    if !matches!(situacao(), Situacao::Pronta(_)) {
        return false;
    }
    REINICIAR.store(true, Ordering::Release);
    true
}

pub(crate) fn aplicar_ao_sair() {
    if isolado() || (!habilitado() && !REINICIAR.load(Ordering::Acquire)) {
        return;
    }
    let plano = estado().lock().ok().and_then(|estado| estado.plano.clone());
    let Some(mut plano) = plano else { return };
    plano.reiniciar = REINICIAR.load(Ordering::Acquire);
    if let Err(erro) = aplicador::agendar(&plano) {
        registrar(&format!("Atualização adiada: {erro:#}"));
    }
}

pub(crate) fn executar_modo_auxiliar() -> Option<i32> {
    let argumentos: Vec<_> = std::env::args().skip(1).collect();
    match argumentos.first().map(String::as_str) {
        Some("--aplicar-atualizacao") => {
            let resultado = argumentos
                .get(1)
                .context("Plano de atualização ausente")
                .and_then(|plano| aplicador::executar(std::path::Path::new(plano)));
            let codigo = i32::from(resultado.is_err());
            registrar(&match resultado {
                Ok(()) => "Atualização aplicada".into(),
                Err(erro) => format!("Falha ao aplicar atualização: {erro:#}"),
            });
            Some(codigo)
        }
        Some("--verificar-atualizacao") => {
            let resultado = github::consultar(VERSAO);
            println!(
                "{}",
                match &resultado {
                    Ok(Some(nova)) => serde_json::json!({"versao_atual": VERSAO, "disponivel": nova.versao}),
                    Ok(None) => serde_json::json!({"versao_atual": VERSAO, "disponivel": null}),
                    Err(erro) => serde_json::json!({"erro": format!("{erro:#}")}),
                }
            );
            Some(i32::from(resultado.is_err()))
        }
        _ => None,
    }
}

fn definir_situacao(situacao: Situacao) {
    if let Ok(mut estado) = estado().lock() {
        estado.situacao = situacao;
    }
}

fn pasta_cache() -> Result<PathBuf> {
    Ok(
        PathBuf::from(std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA ausente")?)
            .join("Zaga")
            .join("atualizacoes"),
    )
}

fn limpar_cache() {
    let Ok(raiz) = pasta_cache().and_then(|raiz| Ok(raiz.canonicalize()?)) else {
        return;
    };
    let Ok(pastas) = std::fs::read_dir(&raiz) else { return };
    for pasta in pastas.flatten() {
        let caminho = pasta.path();
        if pasta.file_name().to_string_lossy().starts_with("sessao_")
            && pasta.file_type().is_ok_and(|tipo| tipo.is_dir() && !tipo.is_symlink())
            && caminho.parent() == Some(raiz.as_path())
        {
            let _ = std::fs::remove_dir_all(caminho);
        }
    }
}

fn registrar(mensagem: &str) {
    let pasta = std::env::var_os("ZAGA_DADOS")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(|local| PathBuf::from(local).join("Zaga")));
    if let Some(pasta) = pasta {
        let _ = std::fs::create_dir_all(&pasta);
        if let Ok(mut arquivo) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(pasta.join("atualizacoes.log"))
        {
            if arquivo.metadata().is_ok_and(|metadados| metadados.len() > 1024 * 1024) {
                let _ = arquivo.set_len(0);
            }
            let _ = writeln!(arquivo, "{mensagem}");
        }
    }
}
