use crate::controlador::Controlador;
use crate::registro::registrar;
use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, OnceLock};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, UnregisterHotKey,
};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW,
    GetCursorPos, GetMessageW, HWND_DESKTOP, IDI_APPLICATION, KillTimer, LoadIconW, MF_SEPARATOR, MF_STRING, MSG,
    PostMessageW, PostQuitMessage, RegisterClassW, SW_SHOWNORMAL, SetForegroundWindow, SetTimer, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_DESTROY, WM_HOTKEY,
    WM_LBUTTONDBLCLK, WM_NULL, WM_RBUTTONUP, WM_TIMER, WNDCLASSW,
};
use windows::core::{HSTRING, PCWSTR, w};
use zaga_nucleo::atalho::Atalho;
use zaga_nucleo::config::DURACOES_CLIPE;

const MENSAGEM_BANDEJA: u32 = WM_APP + 1;
const ID_ICONE: u32 = 1;
const ID_RECURSO_ICONE: usize = 1;
const MENSAGEM_REAPLICAR_ATALHOS: u32 = WM_APP + 2;
const MENSAGEM_SUSPENDER_ATALHOS: u32 = WM_APP + 3;
const ID_TEMPORIZADOR_SUSPENSAO: usize = 1;
const TEMPO_MAXIMO_SUSPENSAO_MS: u32 = 60_000;

const COMANDO_ABRIR: usize = 100;
const COMANDO_PASTA: usize = 101;
const COMANDO_SAIR: usize = 102;
const COMANDO_SALVAR_BASE: usize = 200;

static CONTROLADOR: OnceLock<Arc<Controlador>> = OnceLock::new();
static JANELA: AtomicIsize = AtomicIsize::new(0);
static SUSPENSO: AtomicBool = AtomicBool::new(false);

/// Cria a janela oculta, registra atalhos e o ícone da bandeja e processa mensagens até o encerramento.
pub fn executar(controlador: Arc<Controlador>, pronto: std::sync::mpsc::SyncSender<Result<(), String>>) -> Result<()> {
    assert!(
        CONTROLADOR.set(controlador).is_ok(),
        "janela do motor iniciada duas vezes"
    );
    let janela = criar_janela_oculta()?;
    JANELA.store(janela.0 as isize, Ordering::SeqCst);
    registrar_atalhos(janela);
    adicionar_icone(janela)?;
    let _ = pronto.send(Ok(()));

    let mut mensagem = MSG::default();
    while unsafe { GetMessageW(&mut mensagem, None, 0, 0) }.as_bool() {
        unsafe {
            let _ = TranslateMessage(&mensagem);
            DispatchMessageW(&mensagem);
        }
    }

    remover_icone(janela);
    remover_atalhos(janela);
    let _ = unsafe { DestroyWindow(janela) };
    Ok(())
}

fn criar_janela_oculta() -> Result<HWND> {
    let instancia = unsafe { GetModuleHandleW(None) }?;
    let classe = w!("ZagaMotorJanela");
    let definicao = WNDCLASSW {
        lpfnWndProc: Some(procedimento_janela),
        hInstance: instancia.into(),
        lpszClassName: classe,
        ..Default::default()
    };
    if unsafe { RegisterClassW(&definicao) } == 0 {
        return Err(windows::core::Error::from_thread()).context("falha ao registrar a classe da janela");
    }
    let janela = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            classe,
            w!("Zaga"),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instancia.into()),
            None,
        )
    }
    .context("falha ao criar a janela oculta")?;
    Ok(janela)
}

/// Pede à janela que registre de novo os atalhos da configuração atual. Pode ser chamada de qualquer thread.
pub fn reaplicar_atalhos() {
    postar_para_janela(MENSAGEM_REAPLICAR_ATALHOS, 0);
}

/// Solta ou retoma os atalhos globais, para que a interface consiga ler a combinação que o usuário pressionar.
pub fn suspender_atalhos(suspender: bool) {
    postar_para_janela(MENSAGEM_SUSPENDER_ATALHOS, usize::from(suspender));
}

fn postar_para_janela(mensagem: u32, parametro: usize) {
    let janela = HWND(JANELA.load(Ordering::SeqCst) as *mut _);
    let _ = unsafe { PostMessageW(Some(janela), mensagem, WPARAM(parametro), LPARAM(0)) };
}

fn registrar_atalhos(janela: HWND) {
    let Some(controlador) = CONTROLADOR.get() else {
        return;
    };
    remover_atalhos(janela);
    let mut indisponiveis = Vec::new();
    for (indice, atalho) in controlador.atalhos().iter().enumerate() {
        if let Err(erro) = registrar_atalho(janela, indice, atalho) {
            registrar(format!("atalho {} indisponivel: {erro:#}", atalho.descricao()));
            indisponiveis.push(indice);
        }
    }
    controlador.definir_atalhos_indisponiveis(indisponiveis);
}

fn registrar_atalho(janela: HWND, indice: usize, atalho: &Atalho) -> Result<()> {
    let tecla = atalho.codigo_virtual().context("tecla nao suportada")?;
    let mut modificadores: HOT_KEY_MODIFIERS = MOD_NOREPEAT;
    if atalho.ctrl {
        modificadores |= MOD_CONTROL;
    }
    if atalho.alt {
        modificadores |= MOD_ALT;
    }
    if atalho.shift {
        modificadores |= MOD_SHIFT;
    }
    unsafe { RegisterHotKey(Some(janela), indice as i32, modificadores, tecla) }?;
    Ok(())
}

fn remover_atalhos(janela: HWND) {
    for indice in 0..DURACOES_CLIPE.len() {
        let _ = unsafe { UnregisterHotKey(Some(janela), indice as i32) };
    }
}

fn tratar_suspensao(janela: HWND, suspender: bool) {
    SUSPENSO.store(suspender, Ordering::SeqCst);
    if suspender {
        remover_atalhos(janela);
        unsafe { SetTimer(Some(janela), ID_TEMPORIZADOR_SUSPENSAO, TEMPO_MAXIMO_SUSPENSAO_MS, None) };
    } else {
        let _ = unsafe { KillTimer(Some(janela), ID_TEMPORIZADOR_SUSPENSAO) };
        registrar_atalhos(janela);
    }
}

fn dados_icone(janela: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: janela,
        uID: ID_ICONE,
        ..Default::default()
    }
}

fn adicionar_icone(janela: HWND) -> Result<()> {
    let mut dados = dados_icone(janela);
    dados.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    dados.uCallbackMessage = MENSAGEM_BANDEJA;
    let instancia = unsafe { GetModuleHandleW(None) }?;
    dados.hIcon = unsafe { LoadIconW(Some(instancia.into()), PCWSTR(ID_RECURSO_ICONE as *const u16)) }
        .or_else(|_| unsafe { LoadIconW(None, IDI_APPLICATION) })?;
    for (destino, caractere) in dados.szTip.iter_mut().zip("Zaga".encode_utf16()) {
        *destino = caractere;
    }
    unsafe { Shell_NotifyIconW(NIM_ADD, &dados) }
        .ok()
        .context("falha ao criar o icone da bandeja")
}

fn remover_icone(janela: HWND) {
    let dados = dados_icone(janela);
    let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &dados) };
}

extern "system" fn procedimento_janela(janela: HWND, mensagem: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match mensagem {
        WM_HOTKEY => salvar_pelo_indice(wparam.0),
        MENSAGEM_BANDEJA => tratar_bandeja(janela, lparam.0 as u32),
        MENSAGEM_REAPLICAR_ATALHOS => {
            if !SUSPENSO.load(Ordering::SeqCst) {
                registrar_atalhos(janela);
            }
        }
        MENSAGEM_SUSPENDER_ATALHOS => tratar_suspensao(janela, wparam.0 != 0),
        WM_TIMER => tratar_suspensao(janela, false),
        WM_DESTROY => unsafe { PostQuitMessage(0) },
        mensagem if mensagem == WM_APP + 4 => abrir_interface(),
        _ => return unsafe { DefWindowProcW(janela, mensagem, wparam, lparam) },
    }
    LRESULT(0)
}

fn salvar_pelo_indice(indice: usize) {
    let (Some(segundos), Some(controlador)) = (DURACOES_CLIPE.get(indice), CONTROLADOR.get()) else {
        return;
    };
    if let Err(erro) = controlador.salvar_clipe(*segundos) {
        registrar(format!("nao foi possivel salvar: {erro:#}"));
    }
}

fn tratar_bandeja(janela: HWND, evento: u32) {
    match evento {
        WM_LBUTTONDBLCLK => abrir_interface(),
        WM_RBUTTONUP => mostrar_menu(janela),
        _ => {}
    }
}

fn mostrar_menu(janela: HWND) {
    let Ok(menu) = (unsafe { CreatePopupMenu() }) else {
        return;
    };
    unsafe {
        let _ = AppendMenuW(menu, MF_STRING, COMANDO_ABRIR, w!("Abrir"));
        let _ = AppendMenuW(menu, MF_STRING, COMANDO_PASTA, w!("Pasta de clipes"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let atalhos = CONTROLADOR
            .get()
            .map_or_else(Atalho::padroes, |controlador| controlador.atalhos());
        for (indice, segundos) in DURACOES_CLIPE.iter().enumerate() {
            let rotulo = HSTRING::from(format!("Salvar {segundos}s\t{}", atalhos[indice].descricao()));
            let _ = AppendMenuW(menu, MF_STRING, COMANDO_SALVAR_BASE + indice, &rotulo);
        }
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, COMANDO_SAIR, w!("Sair"));
    }

    let mut cursor = POINT::default();
    let _ = unsafe { GetCursorPos(&mut cursor) };
    let _ = unsafe { SetForegroundWindow(janela) };
    let escolha = unsafe {
        TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_RETURNCMD,
            cursor.x,
            cursor.y,
            None,
            janela,
            None,
        )
    };
    let _ = unsafe { PostMessageW(Some(janela), WM_NULL, WPARAM(0), LPARAM(0)) };
    let _ = unsafe { DestroyMenu(menu) };
    executar_comando(escolha.0 as usize);
}

fn executar_comando(comando: usize) {
    match comando {
        0 => {}
        COMANDO_ABRIR => abrir_interface(),
        COMANDO_PASTA => abrir_pasta_clipes(),
        COMANDO_SAIR => unsafe { PostQuitMessage(0) },
        outro => salvar_pelo_indice(outro.wrapping_sub(COMANDO_SALVAR_BASE)),
    }
}

fn abrir_interface() {
    crate::ABRIR.store(true, Ordering::Release);
}

/// Solicita encerramento na thread responsável pela bandeja.
pub fn encerrar() {
    postar_para_janela(WM_DESTROY, 0);
}

fn abrir_pasta_clipes() {
    let Some(controlador) = CONTROLADOR.get() else {
        return;
    };
    let pasta = controlador.pasta_clipes();
    let _ = std::fs::create_dir_all(pasta);
    let caminho = HSTRING::from(pasta.as_os_str());
    unsafe { ShellExecuteW(Some(HWND_DESKTOP), w!("open"), &caminho, None, None, SW_SHOWNORMAL) };
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    #[ignore = "Registra brevemente uma combinação global no Windows"]
    fn registra_e_libera_combinacao_personalizada() -> Result<()> {
        let atalho = Atalho {
            ctrl: true,
            alt: true,
            shift: true,
            tecla: "f23".into(),
        };
        let janela = HWND::default();
        registrar_atalho(janela, 0x5A01, &atalho)?;
        unsafe { UnregisterHotKey(Some(janela), 0x5A01) }?;
        registrar_atalho(janela, 0x5A01, &atalho)?;
        unsafe { UnregisterHotKey(Some(janela), 0x5A01) }?;
        Ok(())
    }
}
