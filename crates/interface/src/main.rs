#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod aplicativo;
mod atualizador;
mod components;
mod recursos;
mod services;
mod tema;
mod views;

use aplicativo::Aplicativo;
use gpui::{App, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, prelude::*, px, size};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use recursos::Recursos;
use std::time::Duration;
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, WPARAM};
use windows::Win32::Media::MediaFoundation::{MF_VERSION, MFStartup};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, PostMessageW, SW_HIDE, SW_RESTORE, SW_SHOWNOACTIVATE, ShowWindow, WM_APP,
};
use windows::core::HSTRING;

const LARGURA_JANELA: f32 = 1120.0;
const ALTURA_JANELA: f32 = 740.0;
const VARIAVEL_TESTE_SEM_FOCO: &str = "ZAGA_TESTE_SEM_FOCO";

fn main() {
    if let Some(codigo) = atualizador::executar_modo_auxiliar() {
        std::process::exit(codigo);
    }
    if zaga_motor::executar_monitor_steam() {
        return;
    }
    let nome = HSTRING::from(zaga_motor::nome_instancia("Local\\ZagaAplicativoInstancia"));
    let _instancia = unsafe { CreateMutexW(None, false, &nome) }.expect("falha ao verificar instância do Zaga");
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        let classe = HSTRING::from(zaga_motor::nome_instancia("ZagaMotorJanela"));
        if let Ok(janela) = unsafe { FindWindowW(&classe, None) } {
            let _ = unsafe { PostMessageW(Some(janela), WM_APP + 4, WPARAM(0), LPARAM(0)) };
        }
        return;
    }
    zaga_motor::iniciar().expect("falha ao iniciar a captura do Zaga");
    atualizador::iniciar();
    Application::new().with_assets(Recursos).run(|contexto: &mut App| {
        contexto
            .text_system()
            .add_fonts(vec![
                std::borrow::Cow::Borrowed(include_bytes!("../../../assets/fonts/Inter.ttf")),
                std::borrow::Cow::Borrowed(include_bytes!("../../../assets/fonts/SpaceGrotesk.ttf")),
            ])
            .expect("falha ao carregar as fontes da interface");
        assert!(
            unsafe { MFStartup(MF_VERSION, 0) }.is_ok(),
            "Media Foundation indisponivel"
        );
        if !std::env::args().any(|argumento| argumento == "--bandeja") {
            abrir_janela(contexto);
        }
        contexto
            .spawn(async move |contexto| {
                loop {
                    contexto.background_executor().timer(Duration::from_millis(100)).await;
                    if contexto
                        .update(|contexto| {
                            if zaga_motor::encerrado() {
                                contexto.quit();
                                return;
                            }
                            if zaga_motor::consumir_abertura() {
                                abrir_janela(contexto);
                            }
                        })
                        .is_err()
                    {
                        return;
                    }
                }
            })
            .detach();
    });
    atualizador::aplicar_ao_sair();
}

fn abrir_janela(contexto: &mut App) {
    if let Some(janela) = contexto
        .windows()
        .into_iter()
        .find(|janela| janela.downcast::<Aplicativo>().is_some())
    {
        let _ = janela.update(contexto, |_, janela, _| {
            let sem_foco = std::env::var_os(VARIAVEL_TESTE_SEM_FOCO).is_some();
            if let Some(nativa) = janela_nativa(janela) {
                let comando = if sem_foco { SW_SHOWNOACTIVATE } else { SW_RESTORE };
                let _ = unsafe { ShowWindow(nativa, comando) };
            }
            if !sem_foco {
                janela.activate_window();
            }
        });
        return;
    }
    let abrir_em_segundo_plano = std::env::var_os(VARIAVEL_TESTE_SEM_FOCO).is_some();
    let limites = Bounds::centered(None, size(px(LARGURA_JANELA), px(ALTURA_JANELA)), contexto);
    let opcoes = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(limites)),
        titlebar: Some(TitlebarOptions {
            title: Some("ZAGA".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(0.0), px(0.0))),
        }),
        window_min_size: Some(size(px(920.0), px(640.0))),
        show: !abrir_em_segundo_plano,
        focus: !abrir_em_segundo_plano,
        ..Default::default()
    };
    let aberta = contexto.open_window(opcoes, |janela, contexto| {
        let aplicativo = contexto.new(Aplicativo::new);
        let entidade = aplicativo.clone();
        janela.on_window_should_close(contexto, move |janela, contexto| {
            entidade.update(contexto, |aplicativo, contexto| {
                aplicativo.cancelar_gravacao_de_atalho(contexto);
                aplicativo.fechar_editor();
            });
            if let Some(nativa) = janela_nativa(janela) {
                let _ = unsafe { ShowWindow(nativa, SW_HIDE) };
            }
            false
        });
        aplicativo
    });
    assert!(aberta.is_ok(), "falha ao abrir a janela principal");
    if !abrir_em_segundo_plano {
        contexto.activate(true);
    }
}

fn janela_nativa(janela: &gpui::Window) -> Option<HWND> {
    match HasWindowHandle::window_handle(janela).ok()?.as_raw() {
        RawWindowHandle::Win32(identificador) => Some(HWND(identificador.hwnd.get() as *mut _)),
        _ => None,
    }
}
