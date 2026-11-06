use crate::dispositivo::DispositivoGrafico;
use anyhow::{Context, Result};
use std::sync::mpsc::{Receiver, RecvTimeoutError, sync_channel};
use std::time::Duration;
use windows::Foundation::{TimeSpan, TypedEventHandler};
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::WinRT::Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::core::{IInspectable, Interface, factory};
use zaga_nucleo::UNIDADES_POR_SEGUNDO;
use zaga_nucleo::ritmo::RitmoQuadros;

const QUADROS_NO_POOL: i32 = 2;
const DIVISOR_INTERVALO_DO_SISTEMA: i64 = 3;

pub struct Quadro {
    quadro: Direct3D11CaptureFrame,
    pub textura: ID3D11Texture2D,
    pub pts: i64,
    pub largura: u32,
    pub altura: u32,
}

impl Drop for Quadro {
    fn drop(&mut self) {
        let _ = self.quadro.Close();
    }
}

pub enum ResultadoEspera {
    Quadro(Quadro),
    Descartado,
    TelaParada,
}

pub struct Captura {
    dispositivo_winrt: IDirect3DDevice,
    pool: Direct3D11CaptureFramePool,
    sessao: GraphicsCaptureSession,
    aviso_quadro: Receiver<()>,
    tamanho: SizeInt32,
    ritmo: RitmoQuadros,
}

impl Captura {
    pub fn iniciar(dispositivo: &DispositivoGrafico, fps: u32, capturar_cursor: bool) -> Result<Self> {
        assert!(fps > 0, "fps precisa ser positivo");
        let item = item_do_monitor(dispositivo)?;
        let dispositivo_winrt = dispositivo_winrt(dispositivo)?;
        let tamanho = item.Size()?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &dispositivo_winrt,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            QUADROS_NO_POOL,
            tamanho,
        )
        .context("falha ao criar o pool de captura")?;

        let (avisar, aviso_quadro) = sync_channel(1);
        pool.FrameArrived(&TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new(
            move |_, _| {
                let _ = avisar.try_send(());
                Ok(())
            },
        ))?;

        let sessao = pool.CreateCaptureSession(&item)?;
        let intervalo = UNIDADES_POR_SEGUNDO / i64::from(fps);
        configurar_sessao(&sessao, capturar_cursor, intervalo / DIVISOR_INTERVALO_DO_SISTEMA);
        sessao.StartCapture().context("falha ao iniciar a captura")?;

        Ok(Self {
            dispositivo_winrt,
            pool,
            sessao,
            aviso_quadro,
            tamanho,
            ritmo: RitmoQuadros::new(intervalo),
        })
    }

    pub fn tamanho(&self) -> (u32, u32) {
        (self.tamanho.Width as u32, self.tamanho.Height as u32)
    }

    /// Bloqueia até a captura entregar algo ou a espera terminar, o que indica uma tela sem alterações.
    pub fn aguardar_quadro(&mut self, espera: Duration) -> Result<ResultadoEspera> {
        match self.aviso_quadro.recv_timeout(espera) {
            Ok(()) => Ok(self
                .obter_mais_recente()?
                .map_or(ResultadoEspera::Descartado, ResultadoEspera::Quadro)),
            Err(RecvTimeoutError::Timeout) => Ok(ResultadoEspera::TelaParada),
            Err(RecvTimeoutError::Disconnected) => anyhow::bail!("captura encerrada pelo sistema"),
        }
    }

    fn obter_mais_recente(&mut self) -> Result<Option<Quadro>> {
        let mut mais_recente = None;
        while let Ok(quadro) = self.pool.TryGetNextFrame() {
            if let Some(anterior) = mais_recente.replace(quadro) {
                let _ = Direct3D11CaptureFrame::Close(&anterior);
            }
        }
        let Some(quadro) = mais_recente else {
            return Ok(None);
        };

        let tamanho = quadro.ContentSize()?;
        if tamanho != self.tamanho {
            self.recriar_pool(tamanho)?;
            let _ = quadro.Close();
            return Ok(None);
        }

        let pts = quadro.SystemRelativeTime()?.Duration;
        if !self.ritmo.aceitar(pts) {
            let _ = quadro.Close();
            return Ok(None);
        }

        let acesso: IDirect3DDxgiInterfaceAccess = quadro.Surface()?.cast()?;
        let textura: ID3D11Texture2D = unsafe { acesso.GetInterface() }?;
        Ok(Some(Quadro {
            quadro,
            textura,
            pts,
            largura: tamanho.Width as u32,
            altura: tamanho.Height as u32,
        }))
    }

    fn recriar_pool(&mut self, tamanho: SizeInt32) -> Result<()> {
        self.pool.Recreate(
            &self.dispositivo_winrt,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            QUADROS_NO_POOL,
            tamanho,
        )?;
        self.tamanho = tamanho;
        Ok(())
    }
}

impl Drop for Captura {
    fn drop(&mut self) {
        let _ = self.sessao.Close();
        let _ = self.pool.Close();
    }
}

fn item_do_monitor(dispositivo: &DispositivoGrafico) -> Result<GraphicsCaptureItem> {
    let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
    unsafe { interop.CreateForMonitor(dispositivo.monitor) }.context("falha ao capturar o monitor principal")
}

fn dispositivo_winrt(dispositivo: &DispositivoGrafico) -> Result<IDirect3DDevice> {
    let dxgi: IDXGIDevice = dispositivo.dispositivo.cast()?;
    let inspecionavel = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }?;
    Ok(inspecionavel.cast()?)
}

fn configurar_sessao(sessao: &GraphicsCaptureSession, capturar_cursor: bool, intervalo_minimo: i64) {
    let _ = sessao.SetIsCursorCaptureEnabled(capturar_cursor);
    let _ = sessao.SetIsBorderRequired(false);
    let _ = sessao.SetMinUpdateInterval(TimeSpan {
        Duration: intervalo_minimo,
    });
}
