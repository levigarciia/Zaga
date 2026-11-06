use crate::services::midia::{ler_variante_i64, variante_i64};
use anyhow::{Context, Result};
use std::cell::Cell;
use std::path::Path;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{BLACK_BRUSH, ClientToScreen, GetStockObject, HBRUSH};
use windows::Win32::Media::MediaFoundation::{IMFPMediaPlayer, MFP_POSITIONTYPE_100NS, MFPCreateMediaPlayer};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, MA_NOACTIVATE, RegisterClassW, SWP_NOACTIVATE, SWP_NOZORDER,
    SWP_SHOWWINDOW, SetWindowPos, WM_MOUSEACTIVATE, WNDCLASSW, WS_CLIPSIBLINGS, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_POPUP,
};
use windows::core::{HSTRING, w};

extern "system" fn procedimento_video(janela: HWND, mensagem: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if mensagem == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    unsafe { DefWindowProcW(janela, mensagem, wparam, lparam) }
}

/// Reproduz um clipe com o player nativo do Windows, com decodificação de hardware e áudio sincronizado.
///
/// O GPUI desenha acima de janelas filhas, então o vídeo fica em uma janela de propriedade da principal,
/// que precisa ser reposicionada quando a principal se move. Crie e use apenas na thread da interface.
pub struct Reprodutor {
    janela: HWND,
    dona: HWND,
    player: IMFPMediaPlayer,
    area_cliente: Cell<(i32, i32, i32, i32)>,
    area_tela: Cell<(i32, i32, i32, i32)>,
}

impl Reprodutor {
    pub fn abrir(dona: HWND, caminho: &Path) -> Result<Self> {
        let janela = criar_janela_de_video(dona)?;
        let url = HSTRING::from(caminho.as_os_str());
        let mut player = None;
        let criado =
            unsafe { MFPCreateMediaPlayer(&url, true, Default::default(), None, Some(janela), Some(&mut player)) };
        if let Err(erro) = criado {
            let _ = unsafe { DestroyWindow(janela) };
            return Err(erro).context("player de video indisponivel");
        }
        Ok(Self {
            janela,
            dona,
            player: player.context("player ausente")?,
            area_cliente: Cell::new((0, 0, 0, 0)),
            area_tela: Cell::new((0, 0, 0, 0)),
        })
    }

    pub fn tocar(&self) {
        let _ = unsafe { self.player.Play() };
    }

    pub fn pausar(&self) {
        let _ = unsafe { self.player.Pause() };
    }

    pub fn definir_mudo(&self, mudo: bool) {
        let _ = unsafe { self.player.SetMute(mudo) };
    }

    pub fn posicao(&self) -> Option<i64> {
        let valor = unsafe { self.player.GetPosition(&MFP_POSITIONTYPE_100NS) }.ok()?;
        Some(ler_variante_i64(&valor))
    }

    pub fn duracao(&self) -> Option<i64> {
        let valor = unsafe { self.player.GetDuration(&MFP_POSITIONTYPE_100NS) }.ok()?;
        Some(ler_variante_i64(&valor)).filter(|duracao| *duracao > 0)
    }

    pub fn buscar(&self, posicao: i64) {
        let _ = unsafe { self.player.SetPosition(&MFP_POSITIONTYPE_100NS, &variante_i64(posicao)) };
    }

    /// Define a área do vídeo em coordenadas da janela principal.
    pub fn posicionar(&self, x: i32, y: i32, largura: i32, altura: i32) {
        self.area_cliente.set((x, y, largura, altura));
        self.acompanhar_janela();
    }

    /// Reaplica a posição na tela, para seguir a janela principal quando ela é arrastada.
    pub fn acompanhar_janela(&self) {
        let (x, y, largura, altura) = self.area_cliente.get();
        if largura <= 0 || altura <= 0 {
            return;
        }
        let mut origem = POINT { x, y };
        if !unsafe { ClientToScreen(self.dona, &mut origem) }.as_bool() {
            return;
        }
        let nova = (origem.x, origem.y, largura, altura);
        if self.area_tela.replace(nova) == nova {
            return;
        }
        unsafe {
            let _ = SetWindowPos(
                self.janela,
                None,
                origem.x,
                origem.y,
                largura,
                altura,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
            );
            let _ = self.player.UpdateVideo();
        }
    }
}

impl Drop for Reprodutor {
    fn drop(&mut self) {
        unsafe {
            let _ = self.player.Shutdown();
            let _ = DestroyWindow(self.janela);
        }
    }
}

fn criar_janela_de_video(dona: HWND) -> Result<HWND> {
    let instancia = unsafe { GetModuleHandleW(None) }?;
    let classe = w!("ZagaVideo");
    let definicao = WNDCLASSW {
        lpfnWndProc: Some(procedimento_video),
        hInstance: instancia.into(),
        lpszClassName: classe,
        hbrBackground: HBRUSH(unsafe { GetStockObject(BLACK_BRUSH) }.0),
        ..Default::default()
    };
    unsafe { RegisterClassW(&definicao) };
    unsafe {
        CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            classe,
            None,
            WS_POPUP | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            Some(dona),
            None,
            Some(instancia.into()),
            None,
        )
    }
    .context("falha ao criar a janela de video")
}
