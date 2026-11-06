use crate::services::midia::LeitorQuadros;
use anyhow::{Context, Result};
use image::{DynamicImage, RgbaImage};
use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL};

const LARGURA_MINIATURA: u32 = 480;
const LARGURA_TIRA: u32 = 160;
pub const QUADROS_NA_TIRA: usize = 12;

static GERANDO: AtomicBool = AtomicBool::new(false);
static FALHAS: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());

pub fn chave(caminho: &Path, bytes: u64, modificado: SystemTime) -> String {
    let mut hash = DefaultHasher::new();
    caminho.file_name().hash(&mut hash);
    bytes.hash(&mut hash);
    modificado
        .duration_since(UNIX_EPOCH)
        .map_or(0, |tempo| tempo.as_secs())
        .hash(&mut hash);
    format!("{:016x}", hash.finish())
}

pub fn chave_do_arquivo(caminho: &Path) -> Option<String> {
    let metadados = std::fs::metadata(caminho).ok()?;
    Some(chave(caminho, metadados.len(), metadados.modified().ok()?))
}

pub fn ler_cache(cache: &Path, chave: &str) -> (Option<PathBuf>, Option<i64>) {
    let miniatura = cache.join(format!("{chave}.jpg"));
    let duracao = std::fs::read_to_string(cache.join(format!("{chave}.dur")))
        .ok()
        .and_then(|texto| texto.trim().parse().ok());
    (miniatura.exists().then_some(miniatura), duracao)
}

pub fn caminhos_da_tira(cache: &Path, chave: &str) -> Vec<PathBuf> {
    (0..QUADROS_NA_TIRA)
        .map(|indice| cache.join(format!("{chave}_{indice}.jpg")))
        .collect()
}

/// Gera em segundo plano o que falta no cache. Ignora o pedido se já houver uma geração em andamento.
pub fn gerar_miniaturas_pendentes(pendentes: Vec<PathBuf>, cache: PathBuf) {
    let pendentes: Vec<PathBuf> = pendentes.into_iter().filter(|caminho| !ja_falhou(caminho)).collect();
    if pendentes.is_empty() || GERANDO.swap(true, Ordering::SeqCst) {
        return;
    }
    executar_em_segundo_plano(move || {
        for caminho in pendentes {
            if gerar_miniatura(&caminho, &cache).is_err() {
                registrar_falha(caminho);
            }
        }
        GERANDO.store(false, Ordering::SeqCst);
    });
}

fn ja_falhou(caminho: &Path) -> bool {
    FALHAS.lock().is_ok_and(|falhas| falhas.contains(caminho))
}

fn registrar_falha(caminho: PathBuf) {
    if let Ok(mut falhas) = FALHAS.lock() {
        falhas.insert(caminho);
    }
}

/// Gera a tira de quadros usada na linha do tempo do editor.
pub fn gerar_tira(caminho: PathBuf, cache: PathBuf) {
    executar_em_segundo_plano(move || {
        let _ = criar_tira(&caminho, &cache);
    });
}

fn executar_em_segundo_plano(trabalho: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
        }
        trabalho();
    });
}

fn gerar_miniatura(caminho: &Path, cache: &Path) -> Result<()> {
    std::fs::create_dir_all(cache)?;
    let chave = chave_do_arquivo(caminho).context("clipe indisponivel")?;
    let leitor = LeitorQuadros::abrir(caminho)?;
    let imagem = leitor.quadro_em(leitor.duracao() / 3, LARGURA_MINIATURA)?;
    salvar_jpeg(imagem, &cache.join(format!("{chave}.jpg")))?;
    std::fs::write(cache.join(format!("{chave}.dur")), leitor.duracao().to_string())?;
    Ok(())
}

fn criar_tira(caminho: &Path, cache: &Path) -> Result<()> {
    std::fs::create_dir_all(cache)?;
    let chave = chave_do_arquivo(caminho).context("clipe indisponivel")?;
    let leitor = LeitorQuadros::abrir(caminho)?;
    let passo = leitor.duracao() / QUADROS_NA_TIRA as i64;
    for (indice, destino) in caminhos_da_tira(cache, &chave).iter().enumerate() {
        if destino.exists() {
            continue;
        }
        let imagem = leitor.quadro_em(passo * indice as i64 + passo / 2, LARGURA_TIRA)?;
        salvar_jpeg(imagem, destino)?;
    }
    Ok(())
}

fn salvar_jpeg(imagem: RgbaImage, destino: &Path) -> Result<()> {
    let temporario = destino.with_extension("tmp");
    DynamicImage::ImageRgba8(imagem)
        .to_rgb8()
        .save_with_format(&temporario, image::ImageFormat::Jpeg)?;
    std::fs::rename(&temporario, destino)?;
    Ok(())
}
