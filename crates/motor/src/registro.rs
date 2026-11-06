use std::fmt::Display;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static ARQUIVO: OnceLock<Mutex<File>> = OnceLock::new();

pub fn iniciar(pasta_dados: &Path) {
    if let Ok(arquivo) = File::create(pasta_dados.join("motor.log")) {
        let _ = ARQUIVO.set(Mutex::new(arquivo));
    }
    std::panic::set_hook(Box::new(|informacao| registrar(format!("panico: {informacao}"))));
}

pub fn registrar(mensagem: impl Display) {
    let segundos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duracao| duracao.as_secs());
    let linha = format!("[{segundos}] {mensagem}\n");
    #[cfg(debug_assertions)]
    eprint!("{linha}");
    if let Some(Ok(mut arquivo)) = ARQUIVO.get().map(Mutex::lock) {
        let _ = arquivo.write_all(linha.as_bytes());
    }
}
