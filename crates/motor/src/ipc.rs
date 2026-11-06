use crate::controlador::Controlador;
use crate::registro::registrar;
use anyhow::{Result, anyhow, bail};
use std::sync::Arc;
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{FlushFileBuffers, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
use windows::core::HSTRING;
use zaga_nucleo::protocolo::{Pedido, Resposta, caminho_pipe, codificar_linha};

const TAMANHO_BUFFER_PIPE: u32 = 16 * 1024;
const TAMANHO_MAXIMO_PEDIDO: usize = 64 * 1024;

/// Atende a interface por um pipe local, uma conexão por vez, até o processo terminar.
pub fn atender(controlador: Arc<Controlador>, thread_principal: u32) {
    loop {
        if let Err(erro) = atender_conexao(&controlador, thread_principal) {
            registrar(format!("falha no pipe da interface: {erro:#}"));
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}

fn atender_conexao(controlador: &Controlador, thread_principal: u32) -> Result<()> {
    let pipe = PipeServidor::criar()?;
    pipe.aguardar_cliente()?;
    let linha = pipe.ler_linha()?;
    let pedido: Pedido = serde_json::from_str(&linha)?;
    let encerrar = pedido == Pedido::Encerrar;
    let resposta = responder(controlador, pedido);
    pipe.escrever(codificar_linha(&resposta)?.as_bytes())?;
    if encerrar {
        unsafe { PostThreadMessageW(thread_principal, WM_QUIT, Default::default(), Default::default()) }?;
    }
    Ok(())
}

pub(crate) fn responder(controlador: &Controlador, pedido: Pedido) -> Resposta {
    let resultado = match pedido {
        Pedido::ObterEstado => return Resposta::Estado(controlador.estado()),
        Pedido::SalvarClipe { segundos } => controlador.salvar_clipe(segundos),
        Pedido::AplicarConfig(config) => controlador.aplicar_config(config),
        Pedido::SuspenderAtalhos(suspender) => {
            crate::janela::suspender_atalhos(suspender);
            Ok(())
        }
        Pedido::Encerrar => Ok(()),
    };
    match resultado {
        Ok(()) => Resposta::Aceito,
        Err(erro) => Resposta::Falha(format!("{erro:#}")),
    }
}

struct PipeServidor(HANDLE);

impl PipeServidor {
    fn criar() -> Result<Self> {
        let identificador = unsafe {
            CreateNamedPipeW(
                &HSTRING::from(caminho_pipe()),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                TAMANHO_BUFFER_PIPE,
                TAMANHO_BUFFER_PIPE,
                0,
                None,
            )
        };
        if identificador == INVALID_HANDLE_VALUE {
            return Err(anyhow!(windows::core::Error::from_thread()).context("falha ao criar o pipe"));
        }
        Ok(Self(identificador))
    }

    fn aguardar_cliente(&self) -> Result<()> {
        if let Err(erro) = unsafe { ConnectNamedPipe(self.0, None) } {
            let ja_conectado = windows::Win32::Foundation::ERROR_PIPE_CONNECTED.to_hresult();
            if erro.code() != ja_conectado {
                return Err(erro.into());
            }
        }
        Ok(())
    }

    fn ler_linha(&self) -> Result<String> {
        let mut acumulado = Vec::new();
        let mut bloco = [0u8; 4096];
        loop {
            let mut lidos = 0;
            unsafe { ReadFile(self.0, Some(&mut bloco), Some(&mut lidos), None) }?;
            if lidos == 0 {
                bail!("cliente desconectou antes de concluir o pedido");
            }
            acumulado.extend_from_slice(&bloco[..lidos as usize]);
            if acumulado.len() > TAMANHO_MAXIMO_PEDIDO {
                bail!("pedido excede o tamanho maximo");
            }
            if let Some(fim) = acumulado.iter().position(|byte| *byte == b'\n') {
                acumulado.truncate(fim);
                return Ok(String::from_utf8(acumulado)?);
            }
        }
    }

    fn escrever(&self, dados: &[u8]) -> Result<()> {
        let mut escritos = 0;
        unsafe {
            WriteFile(self.0, Some(dados), Some(&mut escritos), None)?;
            FlushFileBuffers(self.0)?;
        }
        Ok(())
    }
}

impl Drop for PipeServidor {
    fn drop(&mut self) {
        unsafe {
            let _ = DisconnectNamedPipe(self.0);
            let _ = CloseHandle(self.0);
        }
    }
}
