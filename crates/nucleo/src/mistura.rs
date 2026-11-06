//! Mistura do microfone ao áudio do sistema em uma única trilha.
//!
//! Trilha única porque Discord, navegadores e a maioria dos players só tocam a primeira trilha de áudio de um MP4.

use std::collections::VecDeque;

const AMOSTRAS_MAXIMAS: usize = 19_200;

/// Fila de amostras PCM de 16 bits vindas do microfone, à espera de serem misturadas.
///
/// O limite de 200 ms descarta o que há de mais antigo, para que a diferença entre os relógios do microfone e
/// da saída de áudio não acumule atraso na voz.
#[derive(Default)]
pub struct FilaMicrofone {
    amostras: VecDeque<i16>,
}

impl FilaMicrofone {
    pub fn empurrar(&mut self, pcm: &[u8]) {
        let novas = pcm.chunks_exact(2).map(|par| i16::from_le_bytes([par[0], par[1]]));
        self.amostras.extend(novas);
        let excedente = self.amostras.len().saturating_sub(AMOSTRAS_MAXIMAS);
        self.amostras.drain(..excedente);
    }

    pub fn empurrar_silencio(&mut self, amostras: usize) {
        self.empurrar(&vec![0; amostras * 2]);
    }

    /// Soma as amostras disponíveis ao PCM de destino, limitando o resultado para não estourar.
    pub fn misturar(&mut self, destino: &mut [u8]) {
        for par in destino.chunks_exact_mut(2) {
            let Some(amostra) = self.amostras.pop_front() else {
                return;
            };
            let somada = i16::from_le_bytes([par[0], par[1]]).saturating_add(amostra);
            par.copy_from_slice(&somada.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn bytes(amostras: &[i16]) -> Vec<u8> {
        amostras.iter().flat_map(|amostra| amostra.to_le_bytes()).collect()
    }

    #[test]
    fn soma_ao_destino_e_consome_a_fila() {
        let mut fila = FilaMicrofone::default();
        fila.empurrar(&bytes(&[10, -20]));
        let mut destino = bytes(&[1, 2, 3]);
        fila.misturar(&mut destino);
        assert_eq!(destino, bytes(&[11, -18, 3]));
        assert!(fila.amostras.is_empty());
    }

    #[test]
    fn soma_nao_estoura() {
        let mut fila = FilaMicrofone::default();
        fila.empurrar(&bytes(&[30_000, -30_000]));
        let mut destino = bytes(&[30_000, -30_000]);
        fila.misturar(&mut destino);
        assert_eq!(destino, bytes(&[i16::MAX, i16::MIN]));
    }

    #[test]
    fn excesso_descarta_o_mais_antigo() {
        let mut fila = FilaMicrofone::default();
        fila.empurrar_silencio(AMOSTRAS_MAXIMAS);
        fila.empurrar(&bytes(&[7]));
        assert_eq!(fila.amostras.len(), AMOSTRAS_MAXIMAS);
        assert_eq!(fila.amostras.back(), Some(&7));
    }
}
