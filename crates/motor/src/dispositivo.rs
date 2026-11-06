use anyhow::{Context, Result, bail};
use windows::Win32::Foundation::{HMODULE, LUID, POINT};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
    ID3D11Device, ID3D11DeviceContext, ID3D11Multithread,
};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIAdapter1, IDXGIDevice, IDXGIFactory1};
use windows::Win32::Graphics::Gdi::{HMONITOR, MONITOR_DEFAULTTOPRIMARY, MonitorFromPoint};
use windows::core::Interface;

const PRIORIDADE_GPU_MAXIMA: i32 = 7;

pub struct DispositivoGrafico {
    pub dispositivo: ID3D11Device,
    pub contexto: ID3D11DeviceContext,
    pub monitor: HMONITOR,
    pub luid: LUID,
}

impl DispositivoGrafico {
    pub fn criar_no_monitor_principal() -> Result<Self> {
        let monitor = unsafe { MonitorFromPoint(POINT::default(), MONITOR_DEFAULTTOPRIMARY) };
        let adaptador = adaptador_do_monitor(monitor)?;
        let luid = unsafe { adaptador.GetDesc1() }?.AdapterLuid;
        let (dispositivo, contexto) = criar_dispositivo(&adaptador)?;

        let _ = unsafe { dispositivo.cast::<ID3D11Multithread>()?.SetMultithreadProtected(true) };
        if let Err(erro) = unsafe {
            dispositivo
                .cast::<IDXGIDevice>()?
                .SetGPUThreadPriority(PRIORIDADE_GPU_MAXIMA)
        } {
            crate::registro::registrar(format!("prioridade de GPU indisponivel: {erro}"));
        }
        Ok(Self {
            dispositivo,
            contexto,
            monitor,
            luid,
        })
    }
}

fn adaptador_do_monitor(monitor: HMONITOR) -> Result<IDXGIAdapter1> {
    let fabrica: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }?;
    for indice_adaptador in 0.. {
        let Ok(adaptador) = (unsafe { fabrica.EnumAdapters1(indice_adaptador) }) else {
            break;
        };
        for indice_saida in 0.. {
            let Ok(saida) = (unsafe { adaptador.EnumOutputs(indice_saida) }) else {
                break;
            };
            if unsafe { saida.GetDesc() }?.Monitor == monitor {
                return Ok(adaptador);
            }
        }
    }
    bail!("nenhum adaptador grafico controla o monitor principal")
}

fn criar_dispositivo(adaptador: &IDXGIAdapter1) -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let niveis: [D3D_FEATURE_LEVEL; 2] = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];
    let mut dispositivo = None;
    let mut contexto = None;
    unsafe {
        D3D11CreateDevice(
            adaptador,
            D3D_DRIVER_TYPE_UNKNOWN,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
            Some(&niveis),
            D3D11_SDK_VERSION,
            Some(&mut dispositivo),
            None,
            Some(&mut contexto),
        )
    }
    .context("falha ao criar o dispositivo D3D11")?;
    Ok((
        dispositivo.context("dispositivo D3D11 ausente")?,
        contexto.context("contexto D3D11 ausente")?,
    ))
}
