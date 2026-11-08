use std::path::PathBuf;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible,
};
use windows::core::{BOOL, PWSTR};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct JogoDetectado {
    pub nome: String,
    pub pid: u32,
    pub executavel: PathBuf,
    pub primeiro_plano: bool,
}

pub(crate) fn detectar(anterior: Option<u32>) -> Option<JogoDetectado> {
    let mut jogos = Vec::<JogoDetectado>::new();
    let _ = unsafe { EnumWindows(Some(visitar), LPARAM((&mut jogos as *mut Vec<JogoDetectado>) as isize)) };
    jogos.sort_by_key(|jogo| (!jogo.primeiro_plano, Some(jogo.pid) != anterior, jogo.pid));
    jogos.into_iter().next()
}

pub(crate) fn continua_em_execucao(jogo: &JogoDetectado) -> bool {
    let Ok(processo) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, jogo.pid) }) else {
        return false;
    };
    let mut codigo = 0;
    let vivo = unsafe { windows::Win32::System::Threading::GetExitCodeProcess(processo, &mut codigo) }.is_ok()
        && codigo == 259;
    let mut buffer = [0u16; 32768];
    let mut tamanho = buffer.len() as u32;
    let mesmo =
        unsafe { QueryFullProcessImageNameW(processo, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut tamanho) }
            .is_ok()
            && PathBuf::from(String::from_utf16_lossy(&buffer[..tamanho as usize])) == jogo.executavel;
    let _ = unsafe { CloseHandle(processo) };
    vivo && mesmo
}

unsafe extern "system" fn visitar(janela: HWND, parametro: LPARAM) -> BOOL {
    let jogos = unsafe { &mut *(parametro.0 as *mut Vec<JogoDetectado>) };
    if let Some(jogo) = identificar_janela(janela) {
        if !jogos.iter().any(|atual| atual.pid == jogo.pid) {
            jogos.push(jogo);
        }
    }
    BOOL(1)
}

fn identificar_janela(janela: HWND) -> Option<JogoDetectado> {
    if !unsafe { IsWindowVisible(janela) }.as_bool() {
        return None;
    }
    let mut area = RECT::default();
    unsafe { GetWindowRect(janela, &mut area) }.ok()?;
    let mut titulo = [0u16; 512];
    let mut classe = [0u16; 256];
    let tamanho_classe = unsafe { GetClassNameW(janela, &mut classe) }.max(0) as usize;
    if !zaga_nucleo::jogos::classe_permitida(&String::from_utf16_lossy(&classe[..tamanho_classe])) {
        return None;
    }
    let tamanho_titulo = unsafe { GetWindowTextW(janela, &mut titulo) }.max(0) as usize;
    if !zaga_nucleo::jogos::janela_permitida(
        &String::from_utf16_lossy(&titulo[..tamanho_titulo]),
        area.right - area.left,
        area.bottom - area.top,
    ) {
        return None;
    }
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(janela, Some(&mut pid)) };
    if pid == 0 || pid == std::process::id() {
        return None;
    }
    let processo = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buffer = [0u16; 32768];
    let mut tamanho = buffer.len() as u32;
    let resultado =
        unsafe { QueryFullProcessImageNameW(processo, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut tamanho) };
    let _ = unsafe { CloseHandle(processo) };
    resultado.ok()?;
    let executavel = PathBuf::from(String::from_utf16_lossy(&buffer[..tamanho as usize]));
    let nome = zaga_nucleo::jogos::identificar(executavel.file_name()?.to_str()?)
        .map(str::to_owned)
        .or_else(|| crate::autoclipagem::steam::manifesto::identificar(&executavel).map(|(_, nome)| nome))?;
    Some(JogoDetectado {
        nome,
        pid,
        executavel,
        primeiro_plano: janela == unsafe { GetForegroundWindow() },
    })
}
