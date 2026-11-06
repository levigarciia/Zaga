use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::fs::{FileExt, OpenOptionsExt};
use std::path::Path;
use std::sync::{Arc, Mutex};
use zaga_nucleo::anel::{AnelPacotes, Armazenamento};

const ACESSO_LEITURA_ESCRITA_REMOCAO: u32 = 0x8000_0000 | 0x4000_0000 | 0x0001_0000;
const REMOVER_AO_FECHAR: u32 = 0x0400_0000;

pub type AnelCompartilhado = Arc<Mutex<AnelPacotes<ArquivoCircular>>>;

pub struct ArquivoCircular {
    arquivo: File,
    capacidade: u64,
}

impl ArquivoCircular {
    pub fn criar(caminho: &Path, capacidade: u64) -> io::Result<Self> {
        let arquivo = OpenOptions::new()
            .write(true)
            .access_mode(ACESSO_LEITURA_ESCRITA_REMOCAO)
            .share_mode(0)
            .custom_flags(REMOVER_AO_FECHAR)
            .create(true)
            .truncate(true)
            .open(caminho)?;
        arquivo.set_len(capacidade)?;
        Ok(Self { arquivo, capacidade })
    }
}

impl Armazenamento for ArquivoCircular {
    fn capacidade(&self) -> u64 {
        self.capacidade
    }

    fn escrever(&mut self, deslocamento: u64, dados: &[u8]) -> io::Result<()> {
        let mut escritos = 0;
        while escritos < dados.len() {
            escritos += self
                .arquivo
                .seek_write(&dados[escritos..], deslocamento + escritos as u64)?;
        }
        Ok(())
    }

    fn ler(&mut self, deslocamento: u64, destino: &mut [u8]) -> io::Result<()> {
        let mut lidos = 0;
        while lidos < destino.len() {
            let parcial = self
                .arquivo
                .seek_read(&mut destino[lidos..], deslocamento + lidos as u64)?;
            if parcial == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            lidos += parcial;
        }
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn arquivo_guarda_os_bytes_e_some_ao_fechar() {
        let caminho = std::env::temp_dir().join(format!("zaga_teste_anel_{}.bin", std::process::id()));
        let mut armazenamento = ArquivoCircular::criar(&caminho, 16).unwrap();
        armazenamento.escrever(10, &[1, 2, 3]).unwrap();
        let mut destino = [0; 3];
        armazenamento.ler(10, &mut destino).unwrap();
        assert_eq!(destino, [1, 2, 3]);
        assert!(caminho.exists());
        drop(armazenamento);
        assert!(!caminho.exists());
    }
}
