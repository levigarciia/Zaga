use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId};
use windows::core::PWSTR;

const ORIGEM_PADRAO: &str = "Clipe";
const TITULOS_IGNORADOS: [&str; 3] = ["zaga", "program manager", "gerenciador de programas"];
const TAMANHO_MAXIMO: usize = 40;

/// Nome da janela em primeiro plano no momento do atalho, usado para identificar o jogo no arquivo.
pub fn origem_em_primeiro_plano() -> String {
    if let Some(jogo) = jogo_em_primeiro_plano() {
        return jogo;
    }
    let mut buffer = [0u16; 256];
    let tamanho = unsafe { GetWindowTextW(GetForegroundWindow(), &mut buffer) };
    let titulo = String::from_utf16_lossy(&buffer[..tamanho.max(0) as usize]);
    origem_a_partir_do_titulo(&titulo)
}

/// Identifica o jogo ativo sem ler a memória do processo ou depender do título da janela.
pub fn jogo_em_primeiro_plano() -> Option<String> {
    let mut identificador = 0;
    unsafe { GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut identificador)) };
    if identificador == 0 {
        return None;
    }
    let processo = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, identificador) }.ok()?;
    let mut caminho = [0u16; 32768];
    let mut tamanho = caminho.len() as u32;
    let resultado =
        unsafe { QueryFullProcessImageNameW(processo, PROCESS_NAME_WIN32, PWSTR(caminho.as_mut_ptr()), &mut tamanho) };
    let _ = unsafe { CloseHandle(processo) };
    resultado.ok()?;
    let caminho = String::from_utf16_lossy(&caminho[..tamanho as usize]);
    zaga_nucleo::jogos::identificar(caminho.rsplit('\\').next()?).map(str::to_owned)
}

fn origem_a_partir_do_titulo(titulo: &str) -> String {
    let permitido: String = titulo
        .chars()
        .filter(|caractere| !caractere.is_control() && !"\\/:*?\"<>|".contains(*caractere))
        .take(TAMANHO_MAXIMO)
        .collect();
    let limpo = permitido.trim().trim_end_matches('.').trim();
    if limpo.is_empty() || TITULOS_IGNORADOS.contains(&limpo.to_lowercase().as_str()) {
        return ORIGEM_PADRAO.to_owned();
    }
    limpo.to_owned()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn remove_caracteres_invalidos_para_nome_de_arquivo() {
        assert_eq!(
            origem_a_partir_do_titulo("Counter-Strike: 2 <dev>"),
            "Counter-Strike 2 dev"
        );
    }

    #[test]
    fn janelas_do_sistema_viram_nome_padrao() {
        assert_eq!(origem_a_partir_do_titulo(""), ORIGEM_PADRAO);
        assert_eq!(origem_a_partir_do_titulo("ZAGA"), ORIGEM_PADRAO);
        assert_eq!(origem_a_partir_do_titulo("Program Manager"), ORIGEM_PADRAO);
    }

    #[test]
    fn titulos_longos_sao_truncados() {
        assert_eq!(origem_a_partir_do_titulo(&"a".repeat(200)).len(), TAMANHO_MAXIMO);
    }
}
