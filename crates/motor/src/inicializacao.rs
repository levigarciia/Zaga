use anyhow::{Context, Result};
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegDeleteKeyValueW, RegSetKeyValueW};
use windows::core::{HSTRING, w};

/// Inclui ou remove o motor da inicialização do Windows do usuário atual.
pub fn definir_inicio_com_windows(ativo: bool) -> Result<()> {
    let chave = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    let valor = w!("ZAGA");
    if !ativo {
        let resultado = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, chave, valor) };
        if resultado != ERROR_SUCCESS && resultado != ERROR_FILE_NOT_FOUND {
            resultado.ok().context("falha ao remover o ZAGA da inicializacao")?;
        }
        return Ok(());
    }

    let executavel = std::env::current_exe().context("caminho do motor indisponivel")?;
    let comando: Vec<u16> = format!("\"{}\" --bandeja", executavel.display())
        .encode_utf16()
        .chain([0])
        .collect();
    let resultado = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            chave,
            &HSTRING::from("ZAGA"),
            REG_SZ.0,
            Some(comando.as_ptr().cast()),
            (comando.len() * 2) as u32,
        )
    };
    resultado.ok().context("falha ao incluir o ZAGA na inicializacao")
}
