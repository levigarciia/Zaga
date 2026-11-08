use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

type Cache = BTreeMap<PathBuf, (Instant, Option<(u32, String)>)>;

pub(crate) fn identificar(executavel: &Path) -> Option<(u32, String)> {
    let common = executavel.ancestors().find(|p| {
        p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("common"))
            && p.parent()
                .and_then(Path::file_name)
                .is_some_and(|n| n.eq_ignore_ascii_case("steamapps"))
    })?;
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Mutex::default).lock().ok()?;
    if let Some((instante, resultado)) = cache.get(executavel) {
        if instante.elapsed() < Duration::from_secs(30) {
            return resultado.clone();
        }
    }
    let resultado = ler_manifestos(executavel, common);
    if cache.len() >= 512 {
        cache.clear();
    }
    cache.insert(executavel.to_owned(), (Instant::now(), resultado.clone()));
    resultado
}

fn ler_manifestos(executavel: &Path, common: &Path) -> Option<(u32, String)> {
    let instalacao = executavel
        .strip_prefix(common)
        .ok()?
        .components()
        .next()?
        .as_os_str()
        .to_str()?;
    for entrada in std::fs::read_dir(common.parent()?).ok()?.flatten() {
        let caminho = entrada.path();
        let Some(nome) = caminho.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !nome.starts_with("appmanifest_") || !nome.ends_with(".acf") {
            continue;
        }
        if entrada.metadata().map_or(true, |m| m.len() > 256 * 1024) {
            continue;
        }
        let Ok(texto) = std::fs::read_to_string(caminho) else {
            continue;
        };
        if let Some(jogo) = interpretar(&texto, instalacao) {
            return Some(jogo);
        }
    }
    None
}

fn interpretar(texto: &str, instalacao: &str) -> Option<(u32, String)> {
    let campos = regex::Regex::new(r#""([^"\r\n]+)"\s+"([^"\r\n]*)""#).ok()?;
    let mut app = None;
    let mut nome = None;
    let mut diretorio = None;
    for par in campos.captures_iter(texto) {
        match &par[1] {
            "appid" => app = par[2].parse::<u32>().ok().filter(|id| *id > 0),
            "name" => nome = Some(par[2].to_owned()),
            "installdir" => diretorio = Some(par[2].to_owned()),
            _ => {}
        }
    }
    if !diretorio?.eq_ignore_ascii_case(instalacao) {
        return None;
    }
    Some((app?, nome.filter(|n| !n.trim().is_empty())?))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn associa_o_manifesto_a_instalacao_correta() {
        let texto = "\"appid\" \"620\"\n\"name\" \"Portal 2\"\n\"installdir\" \"Portal 2\"";
        assert_eq!(interpretar(texto, "portal 2"), Some((620, "Portal 2".into())));
        assert_eq!(interpretar(texto, "Outro jogo"), None);
        assert_eq!(interpretar(&texto.replace("620", "0"), "Portal 2"), None);
    }
}
