//! Catálogo local para identificar jogos pelo nome do executável em primeiro plano.

/// Retorna o nome conhecido do jogo. Processos desconhecidos não são classificados como jogos.
pub fn identificar(executavel: &str) -> Option<&'static str> {
    match executavel.to_ascii_lowercase().as_str() {
        "cs2.exe" => Some("Counter Strike 2"),
        "valorant-win64-shipping.exe" => Some("VALORANT"),
        "league of legends.exe" => Some("League of Legends"),
        "fortniteclient-win64-shipping.exe" => Some("Fortnite"),
        "r5apex.exe" | "r5apex_dx12.exe" => Some("Apex Legends"),
        "rocketleague.exe" => Some("Rocket League"),
        "overwatch.exe" => Some("Overwatch 2"),
        "rainbowsix.exe" | "rainbowsix_vulkan.exe" | "rainbowsix_dx12.exe" => Some("Rainbow Six Siege"),
        "dota2.exe" => Some("Dota 2"),
        "pubg.exe" | "tslgame.exe" => Some("PUBG"),
        "gta5.exe" | "gta5_enhanced.exe" => Some("GTA V"),
        _ => None,
    }
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
}
