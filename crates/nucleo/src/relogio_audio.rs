//! Mantém o áudio alinhado ao relógio do vídeo.
//!
//! A captura de loopback não entrega pacotes durante o silêncio e o relógio da placa de som deriva em relação ao
//! relógio do sistema. Sem correção, sessões longas terminariam com áudio e vídeo dessincronizados.

use crate::UNIDADES_POR_SEGUNDO;

const TOLERANCIA: i64 = UNIDADES_POR_SEGUNDO / 25;
const LIMITE_REANCORAGEM: i64 = 5 * UNIDADES_POR_SEGUNDO;

/// Correção a aplicar antes de consumir o próximo bloco de amostras.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Correcao {
    Nenhuma,
    InserirSilencio { quadros: u32 },
    DescartarQuadros { quadros: u32 },
}

/// Gera os carimbos de tempo do áudio a partir da contagem de amostras, comparando-os ao relógio do sistema.
///
/// Crie um por sessão de captura de áudio.
pub struct RelogioAudio {
    taxa: u32,
    origem: Option<i64>,
    quadros_emitidos: u64,
}

impl RelogioAudio {
    pub fn new(taxa: u32) -> Self {
        assert!(taxa > 0, "taxa de amostragem precisa ser positiva");
        Self {
            taxa,
            origem: None,
            quadros_emitidos: 0,
        }
    }

    /// Compara o instante real de um bloco com a posição esperada e informa como corrigir o desvio.
    pub fn avaliar(&mut self, instante_real: i64) -> Correcao {
        let esperado = self.proximo_pts(instante_real);
        let desvio = instante_real - esperado;
        if desvio.abs() > LIMITE_REANCORAGEM {
            self.origem = Some(instante_real);
            self.quadros_emitidos = 0;
            return Correcao::Nenhuma;
        }
        if desvio > TOLERANCIA {
            return Correcao::InserirSilencio {
                quadros: self.quadros_em(desvio),
            };
        }
        if desvio < -TOLERANCIA {
            return Correcao::DescartarQuadros {
                quadros: self.quadros_em(-desvio),
            };
        }
        Correcao::Nenhuma
    }

    /// Registra quadros emitidos e devolve o carimbo de tempo do primeiro deles.
    pub fn emitir(&mut self, quadros: u32, instante_real: i64) -> i64 {
        let pts = self.proximo_pts(instante_real);
        self.quadros_emitidos += u64::from(quadros);
        pts
    }

    fn proximo_pts(&mut self, instante_real: i64) -> i64 {
        let origem = *self.origem.get_or_insert(instante_real);
        origem + (self.quadros_emitidos as i128 * i128::from(UNIDADES_POR_SEGUNDO) / i128::from(self.taxa)) as i64
    }

    fn quadros_em(&self, unidades: i64) -> u32 {
        (i128::from(unidades) * i128::from(self.taxa) / i128::from(UNIDADES_POR_SEGUNDO)) as u32
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const TAXA: u32 = 48_000;
    const SEGUNDO: i64 = UNIDADES_POR_SEGUNDO;

    #[test]
    fn fluxo_regular_nao_precisa_de_correcao() {
        let mut relogio = RelogioAudio::new(TAXA);
        assert_eq!(relogio.avaliar(SEGUNDO), Correcao::Nenhuma);
        assert_eq!(relogio.emitir(TAXA, SEGUNDO), SEGUNDO);
        assert_eq!(relogio.avaliar(2 * SEGUNDO), Correcao::Nenhuma);
        assert_eq!(relogio.emitir(480, 2 * SEGUNDO), 2 * SEGUNDO);
    }

    #[test]
    fn lacuna_de_silencio_e_preenchida() {
        let mut relogio = RelogioAudio::new(TAXA);
        relogio.avaliar(0);
        relogio.emitir(TAXA, 0);
        assert_eq!(
            relogio.avaliar(3 * SEGUNDO),
            Correcao::InserirSilencio { quadros: 2 * TAXA }
        );
        relogio.emitir(2 * TAXA, 3 * SEGUNDO);
        assert_eq!(relogio.avaliar(3 * SEGUNDO), Correcao::Nenhuma);
    }

    #[test]
    fn audio_adiantado_tem_quadros_descartados() {
        let mut relogio = RelogioAudio::new(TAXA);
        relogio.avaliar(0);
        relogio.emitir(2 * TAXA, 0);
        assert_eq!(relogio.avaliar(SEGUNDO), Correcao::DescartarQuadros { quadros: TAXA });
        assert_eq!(relogio.emitir(0, SEGUNDO), 2 * SEGUNDO);
    }

    #[test]
    fn longa_interrupcao_recomeca_a_linha_do_tempo() {
        let mut relogio = RelogioAudio::new(TAXA);
        relogio.avaliar(0);
        relogio.emitir(TAXA, 0);
        assert_eq!(relogio.avaliar(3600 * SEGUNDO), Correcao::Nenhuma);
        assert_eq!(relogio.emitir(480, 3600 * SEGUNDO), 3600 * SEGUNDO);
    }

    #[test]
    fn pequena_variacao_e_tolerada() {
        let mut relogio = RelogioAudio::new(TAXA);
        relogio.avaliar(0);
        relogio.emitir(TAXA, 0);
        assert_eq!(relogio.avaliar(SEGUNDO + TOLERANCIA), Correcao::Nenhuma);
    }
}
