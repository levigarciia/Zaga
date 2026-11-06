use anyhow::{Context, Result, bail};
use gpui::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{FILETIME, HANDLE, HGLOBAL, HWND, SYSTEMTIME};
use windows::Win32::Storage::FileSystem::FileTimeToLocalFileTime;
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_HDROP;
use windows::Win32::System::Time::FileTimeToSystemTime;
use windows::Win32::UI::Shell::{
    DROPFILES, FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, SHFILEOPSTRUCTW,
    SHFileOperationW,
};
use windows::core::PCWSTR;

const SEGUNDOS_ATE_1970_EM_UNIDADES: u64 = 116_444_736_000_000_000;
const UNIDADES_POR_SEGUNDO: u64 = 10_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DataLocal {
    pub ano: u16,
    pub mes: u16,
    pub dia: u16,
    pub hora: u16,
    pub minuto: u16,
}

pub fn data_local(instante: SystemTime) -> Option<DataLocal> {
    let segundos = instante.duration_since(UNIX_EPOCH).ok()?.as_secs();
    let unidades = segundos * UNIDADES_POR_SEGUNDO + SEGUNDOS_ATE_1970_EM_UNIDADES;
    let universal = FILETIME {
        dwLowDateTime: unidades as u32,
        dwHighDateTime: (unidades >> 32) as u32,
    };
    let mut local = FILETIME::default();
    let mut data = SYSTEMTIME::default();
    unsafe {
        FileTimeToLocalFileTime(&universal, &mut local).ok()?;
        FileTimeToSystemTime(&local, &mut data).ok()?;
    }
    Some(DataLocal {
        ano: data.wYear,
        mes: data.wMonth,
        dia: data.wDay,
        hora: data.wHour,
        minuto: data.wMinute,
    })
}

pub fn identificador_da_janela(janela: &Window) -> Result<HWND> {
    let manipulador =
        HasWindowHandle::window_handle(janela).map_err(|erro| anyhow::anyhow!("janela indisponivel: {erro}"))?;
    match manipulador.as_raw() {
        RawWindowHandle::Win32(win32) => Ok(HWND(win32.hwnd.get() as *mut _)),
        _ => bail!("janela nao e do Windows"),
    }
}

/// Envia arquivos para a Lixeira, de onde o usuário ainda pode recuperá-los.
pub fn enviar_para_lixeira(caminhos: &[&Path]) -> Result<()> {
    let mut lista = lista_com_terminador_duplo(caminhos);
    let mut operacao = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR(lista.as_mut_ptr()),
        fFlags: (FOF_ALLOWUNDO.0 | FOF_NOCONFIRMATION.0 | FOF_SILENT.0 | FOF_NOERRORUI.0) as u16,
        ..Default::default()
    };
    let resultado = unsafe { SHFileOperationW(&mut operacao) };
    if resultado != 0 {
        bail!("falha ao enviar para a Lixeira (codigo {resultado})");
    }
    Ok(())
}

/// Copia o arquivo para a área de transferência, pronto para colar no Discord ou no Explorador.
pub fn copiar_arquivo_para_area_de_transferencia(caminho: &Path) -> Result<()> {
    let lista = lista_com_terminador_duplo(&[caminho]);
    let tamanho_lista = lista.len() * size_of::<u16>();
    let tamanho_total = size_of::<DROPFILES>() + tamanho_lista;
    unsafe {
        let memoria: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, tamanho_total)?;
        let destino = GlobalLock(memoria);
        if destino.is_null() {
            bail!("memoria indisponivel para a area de transferencia");
        }
        let cabecalho = DROPFILES {
            pFiles: size_of::<DROPFILES>() as u32,
            fWide: true.into(),
            ..Default::default()
        };
        destino.cast::<DROPFILES>().write_unaligned(cabecalho);
        std::ptr::copy_nonoverlapping(
            lista.as_ptr().cast::<u8>(),
            destino.cast::<u8>().add(size_of::<DROPFILES>()),
            tamanho_lista,
        );
        let _ = GlobalUnlock(memoria);

        OpenClipboard(None).context("area de transferencia ocupada")?;
        let resultado = EmptyClipboard()
            .and_then(|()| SetClipboardData(u32::from(CF_HDROP.0), Some(HANDLE(memoria.0))).map(|_| ()));
        let _ = CloseClipboard();
        resultado.context("falha ao copiar o clipe")?;
    }
    Ok(())
}

fn lista_com_terminador_duplo(caminhos: &[&Path]) -> Vec<u16> {
    let mut lista: Vec<u16> = Vec::new();
    for caminho in caminhos {
        lista.extend(caminho.as_os_str().encode_wide());
        lista.push(0);
    }
    lista.push(0);
    lista
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn lista_de_arquivos_termina_com_dois_zeros() {
        let lista = lista_com_terminador_duplo(&[Path::new("a"), Path::new("b")]);
        assert_eq!(lista, [u16::from(b'a'), 0, u16::from(b'b'), 0, 0]);
    }
}
