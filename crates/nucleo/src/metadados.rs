use crate::autoclipagem::Destaque;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Localiza os dados privados de um clipe pelo caminho completo do vídeo.
pub fn caminho_metadados(pasta_dados: &Path, clipe: &Path) -> PathBuf {
    let caminho = clipe.canonicalize().unwrap_or_else(|_| clipe.to_path_buf());
    let mut chave = 0xcbf29ce484222325u64;
    for byte in caminho.as_os_str().as_encoded_bytes() {
        chave = (chave ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
    pasta_dados.join("metadados").join(format!("{chave:016x}.json"))
}

/// Persiste os dados fora da pasta de vídeos, concluindo a escrita por renomeação.
pub fn salvar(pasta_dados: &Path, clipe: &Path, destaque: &Destaque) -> io::Result<()> {
    let destino = caminho_metadados(pasta_dados, clipe);
    std::fs::create_dir_all(destino.parent().unwrap())?;
    let temporario = destino.with_extension("json.tmp");
    std::fs::write(&temporario, serde_json::to_vec_pretty(destaque)?)?;
    std::fs::rename(temporario, destino)
}

fn ler_arquivo(caminho: &Path) -> io::Result<Destaque> {
    let mut dados = Vec::new();
    std::fs::File::open(caminho)?
        .take(64 * 1024 + 1)
        .read_to_end(&mut dados)?;
    if dados.len() > 64 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Metadados muito grandes"));
    }
    Ok(serde_json::from_slice(&dados)?)
}

/// Lê os dados e migra arquivos antigos somente após confirmar a cópia interna.
pub fn ler(pasta_dados: &Path, clipe: &Path) -> Option<Destaque> {
    let destino = caminho_metadados(pasta_dados, clipe);
    let legado = clipe.with_extension("json");
    let antigo = ler_arquivo(&legado).ok();
    if let Ok(destaque) = ler_arquivo(&destino) {
        if antigo.as_ref() == Some(&destaque) {
            let _ = std::fs::remove_file(legado);
        }
        return Some(destaque);
    }
    let destaque = antigo?;
    if !destino.exists()
        && salvar(pasta_dados, clipe, &destaque).is_ok()
        && ler_arquivo(&destino).ok().as_ref() == Some(&destaque)
    {
        let _ = std::fs::remove_file(legado);
    }
    Some(destaque)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn migracao_preserva_legado_quando_armazenamento_falha() {
        let pasta = std::env::temp_dir().join(format!(
            "zaga_metadados_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&pasta).unwrap();
        let clipe = pasta.join("clipe.mp4");
        let dados = pasta.join("dados");
        std::fs::write(&dados, b"bloqueado").unwrap();
        let destaque: Destaque = serde_json::from_value(serde_json::json!({
            "titulo":"Jogada", "jogo":"Fortnite", "partida":"p", "rodada":"1",
            "inicio_ms":1, "fim_ms":2, "eliminacoes":1, "mapa":"", "vitoria":false
        }))
        .unwrap();
        let legado = clipe.with_extension("json");
        std::fs::write(&legado, serde_json::to_vec(&destaque).unwrap()).unwrap();
        assert_eq!(ler(&dados, &clipe), Some(destaque.clone()));
        assert!(legado.exists());
        std::fs::remove_file(&dados).unwrap();
        assert_eq!(ler(&dados, &clipe), Some(destaque.clone()));
        assert!(!legado.exists());
        assert_eq!(ler(&dados, &clipe), Some(destaque));
        assert_ne!(
            caminho_metadados(&dados, &clipe),
            caminho_metadados(&dados, &pasta.join("outro/clipe.mp4"))
        );
        std::fs::remove_dir_all(pasta).unwrap();
    }
}
