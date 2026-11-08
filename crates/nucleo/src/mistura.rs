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

    /// Use na trilha final para priorizar a voz sobre o áudio do sistema.
    pub fn misturar(&mut self, destino: &mut [u8]) {
        for par in destino.chunks_exact_mut(2) {
            let amostra = i32::from(self.amostras.pop_front().unwrap_or(0));
            let sistema = i32::from(i16::from_le_bytes([par[0], par[1]]));
            let somada = (sistema / 2 + amostra * 3 / 2).clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
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
    fn prioriza_voz_e_consome_a_fila() {
        let mut fila = FilaMicrofone::default();
        fila.empurrar(&bytes(&[10, -20]));
        let mut destino = bytes(&[1, 2, 3]);
        fila.misturar(&mut destino);
        assert_eq!(destino, bytes(&[15, -29, 1]));
        assert!(fila.amostras.is_empty());
    }

    #[test]
    fn reduz_sistema_mesmo_sem_amostras_do_microfone() {
        let mut fila = FilaMicrofone::default();
        let mut destino = bytes(&[10_000, -10_000]);
        fila.misturar(&mut destino);
        assert_eq!(destino, bytes(&[5_000, -5_000]));
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
