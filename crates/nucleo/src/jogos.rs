//! Catálogo versionado para reconhecer processos e filtrar janelas transitórias.

use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Catalogo {
    versao: u32,
    titulos_ignorados: Vec<String>,
    classes_ignoradas: Vec<String>,
    jogos: Vec<RegraJogo>,
}

#[derive(Deserialize)]
struct RegraJogo {
    nome: String,
    executaveis: Vec<String>,
}

fn catalogo() -> &'static Catalogo {
    static CATALOGO: OnceLock<Catalogo> = OnceLock::new();
    CATALOGO.get_or_init(|| {
        let catalogo: Catalogo =
            serde_json::from_str(include_str!("../../../assets/jogos.json")).expect("Catálogo de jogos inválido");
        assert_eq!(catalogo.versao, 1, "Versão de catálogo incompatível");
        assert!(
            catalogo
                .jogos
                .iter()
                .all(|j| !j.nome.is_empty() && !j.executaveis.is_empty())
        );
        catalogo
    })
}

/// Retorna o nome conhecido do jogo. Processos desconhecidos não são classificados como jogos.
pub fn identificar(executavel: &str) -> Option<&'static str> {
    catalogo()
        .jogos
        .iter()
        .find(|jogo| {
            jogo.executaveis
                .iter()
                .any(|nome| nome.eq_ignore_ascii_case(executavel))
        })
        .map(|jogo| jogo.nome.as_str())
}

/// Disponibiliza os nomes reconhecidos para agrupar clipes na biblioteca.
pub fn nomes_suportados() -> impl Iterator<Item = &'static str> {
    catalogo().jogos.iter().map(|jogo| jogo.nome.as_str())
}

/// Descarta launchers e telas de carregamento sem exigir que o jogo esteja em primeiro plano.
pub fn janela_permitida(titulo: &str, largura: i32, altura: i32) -> bool {
    let titulo = titulo.to_lowercase();
    largura >= 320
        && altura >= 200
        && !catalogo()
            .titulos_ignorados
            .iter()
            .any(|ignorado| titulo.contains(ignorado))
}

/// Ignora classes de diálogos e controles que não representam a superfície do jogo.
pub fn classe_permitida(classe: &str) -> bool {
    !catalogo()
        .classes_ignoradas
        .iter()
        .any(|ignorada| ignorada.eq_ignore_ascii_case(classe))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn reconhece_executaveis_sem_depender_do_titulo() {
        assert_eq!(identificar("CS2.EXE"), Some("Counter Strike 2"));
        assert_eq!(identificar("VALORANT-Win64-Shipping.exe"), Some("VALORANT"));
        assert_eq!(identificar("r5apex_dx12.exe"), Some("Apex Legends"));
    }

    #[test]
    fn nao_confunde_navegador_launcher_ou_java_com_jogo() {
        for executavel in [
            "chrome.exe",
            "steam.exe",
            "RiotClientServices.exe",
            "javaw.exe",
            "cs2.exe.bak",
            "",
        ] {
            assert_eq!(identificar(executavel), None);
        }
    }

    #[test]
    fn rejeita_splash_e_launcher_sem_rejeitar_a_janela_do_jogo() {
        assert!(!janela_permitida("Fortnite Splash", 800, 600));
        assert!(!janela_permitida("Launcher", 800, 600));
        assert!(!janela_permitida("CS2", 160, 30));
        assert!(janela_permitida("Counter Strike 2", 1920, 1080));
        assert!(!classe_permitida("#32770"));
        assert!(classe_permitida("SDL_app"));
    }
}
