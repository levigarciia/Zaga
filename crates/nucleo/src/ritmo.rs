//! Limita a taxa de quadros mantendo a média alvo em monitores de qualquer frequência.
//!
//! Um simples intervalo mínimo entre quadros reduz a taxa real: a 165 Hz, um alvo de 60 fps só encontra quadros
//! aceitáveis a cada 18,18 ms e resulta em 55 fps. Medir contra uma grade de tempo compensa esse arredondamento.

/// Decide quais quadros capturados devem seguir para o encoder.
///
/// Crie um por sessão e consulte [`RitmoQuadros::aceitar`] para cada quadro entregue pela captura.
pub struct RitmoQuadros {
    intervalo: i64,
    proximo_alvo: i64,
    ultimo_aceito: Option<i64>,
}

impl RitmoQuadros {
    pub fn new(intervalo: i64) -> Self {
        assert!(intervalo > 0, "intervalo entre quadros precisa ser positivo");
        Self {
            intervalo,
            proximo_alvo: 0,
            ultimo_aceito: None,
        }
    }

    /// Informa se o quadro deve ser usado, avançando a grade de tempo quando ele é aceito.
    pub fn aceitar(&mut self, pts: i64) -> bool {
        let Some(ultimo) = self.ultimo_aceito else {
            self.registrar_aceite(pts, pts + self.intervalo);
            return true;
        };
        let cedo_para_a_grade = pts < self.proximo_alvo - self.intervalo / 4;
        let colado_ao_anterior = pts - ultimo < self.intervalo / 2;
        if cedo_para_a_grade || colado_ao_anterior {
            return false;
        }
        let atrasado_demais = pts - self.proximo_alvo >= self.intervalo;
        let base = if atrasado_demais { pts } else { self.proximo_alvo };
        self.registrar_aceite(pts, base + self.intervalo);
        true
    }

    fn registrar_aceite(&mut self, pts: i64, proximo_alvo: i64) {
        self.ultimo_aceito = Some(pts);
        self.proximo_alvo = proximo_alvo;
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::UNIDADES_POR_SEGUNDO as SEGUNDO;

    fn quadros_aceitos_em_um_segundo(frequencia_monitor: i64, fps: i64) -> i64 {
        let mut ritmo = RitmoQuadros::new(SEGUNDO / fps);
        (0..frequencia_monitor)
            .filter(|indice| ritmo.aceitar(indice * SEGUNDO / frequencia_monitor))
            .count() as i64
    }

    #[test]
    fn mantem_a_media_em_frequencias_comuns() {
        for frequencia in [60, 75, 120, 144, 165, 240, 360] {
            let aceitos = quadros_aceitos_em_um_segundo(frequencia, 60);
            assert!(
                (59..=61).contains(&aceitos),
                "{frequencia} Hz resultou em {aceitos} fps"
            );
        }
    }

    #[test]
    fn recuperar_atraso_nao_gera_quadros_colados() {
        let intervalo = SEGUNDO / 60;
        let mut ritmo = RitmoQuadros::new(intervalo);
        let mut ultimo = 0;
        assert!(ritmo.aceitar(0));
        for pts in [18, 36, 66, 72, 78, 84, 90, 96, 102].map(|ms| ms * SEGUNDO / 1000) {
            if ritmo.aceitar(pts) {
                assert!(pts - ultimo >= intervalo / 2, "intervalo de {} unidades", pts - ultimo);
                ultimo = pts;
            }
        }
    }

    #[test]
    fn nao_excede_a_frequencia_do_monitor() {
        assert_eq!(quadros_aceitos_em_um_segundo(60, 120), 60);
    }

    #[test]
    fn fonte_lenta_nao_gera_rajada_depois_de_uma_pausa() {
        let mut ritmo = RitmoQuadros::new(SEGUNDO / 60);
        assert!(ritmo.aceitar(0));
        assert!(ritmo.aceitar(10 * SEGUNDO));
        assert!(!ritmo.aceitar(10 * SEGUNDO + SEGUNDO / 240));
    }
}
