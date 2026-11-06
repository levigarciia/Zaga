//! Preferências do usuário e cálculo dos parâmetros derivados delas.

use crate::UNIDADES_POR_SEGUNDO;
use crate::atalho::Atalho;
use serde::{Deserialize, Serialize};

const MARGEM_RETENCAO_SEGUNDOS: u64 = 10;
const FOLGA_PICOS_VBR: f64 = 1.3;
const BITRATE_MINIMO: u32 = 4_000_000;
const BITRATE_MAXIMO: u32 = 150_000_000;

/// Taxa do áudio AAC em bits por segundo.
pub const BITRATE_AUDIO: u32 = 160_000;

/// Durações de clipe oferecidas ao usuário, em segundos.
pub const DURACOES_CLIPE: [u32; 3] = [30, 60, 120];

/// Codec de vídeo usado pelo encoder de hardware.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Codec {
    H264,
    Hevc,
}

/// Equilíbrio entre tamanho do arquivo e nitidez.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Qualidade {
    Economica,
    Alta,
    Maxima,
}

impl Qualidade {
    fn bits_por_pixel(self) -> f64 {
        match self {
            Self::Economica => 0.09,
            Self::Alta => 0.14,
            Self::Maxima => 0.22,
        }
    }
}

/// Altura máxima do vídeo gravado. Reduzir diminui o tamanho dos clipes e o trabalho do encoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resolucao {
    Nativa,
    P1440,
    P1080,
    P720,
}

impl Resolucao {
    /// Calcula o tamanho de saída para uma tela, preservando a proporção e sem nunca ampliar.
    ///
    /// O resultado é sempre par, como exige o formato NV12.
    pub fn aplicar(self, largura: u32, altura: u32) -> (u32, u32) {
        assert!(largura > 0 && altura > 0, "dimensões da tela precisam ser positivas");
        let limite = match self {
            Self::Nativa => altura,
            Self::P1440 => 1440,
            Self::P1080 => 1080,
            Self::P720 => 720,
        };
        if altura <= limite {
            return (largura & !1, altura & !1);
        }
        let largura_escalada = u64::from(largura) * u64::from(limite) / u64::from(altura);
        (largura_escalada as u32 & !1, limite & !1)
    }
}

/// Preferências persistidas do motor de captura.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub captura_ativa: bool,
    pub duracao_clipe: u32,
    pub resolucao: Resolucao,
    pub fps: u32,
    pub qualidade: Qualidade,
    pub codec: Codec,
    pub capturar_cursor: bool,
    pub capturar_audio: bool,
    pub capturar_microfone: bool,
    pub retencao_segundos: u32,
    pub iniciar_com_windows: bool,
    pub atalhos: [Atalho; 3],
}

impl Default for Config {
    fn default() -> Self {
        Self {
            captura_ativa: true,
            duracao_clipe: 30,
            resolucao: Resolucao::Nativa,
            fps: 60,
            qualidade: Qualidade::Alta,
            codec: Codec::H264,
            capturar_cursor: true,
            capturar_audio: true,
            capturar_microfone: false,
            retencao_segundos: 120,
            iniciar_com_windows: false,
            atalhos: Atalho::padroes(),
        }
    }
}

impl Config {
    /// Informa se a mudança exige reiniciar a captura, o que descarta o buffer atual.
    ///
    /// Use para que ajustes sem efeito na gravação não apaguem os últimos segundos do usuário.
    pub fn requer_reinicio(&self, nova: &Config) -> bool {
        self.captura_ativa != nova.captura_ativa
            || self.resolucao != nova.resolucao
            || self.fps != nova.fps
            || self.qualidade != nova.qualidade
            || self.codec != nova.codec
            || self.capturar_cursor != nova.capturar_cursor
            || self.capturar_audio != nova.capturar_audio
            || self.capturar_microfone != nova.capturar_microfone
            || self.retencao_segundos != nova.retencao_segundos
    }

    /// Calcula o bitrate médio do vídeo para uma resolução, em bits por segundo.
    pub fn bitrate_video(&self, largura: u32, altura: u32) -> u32 {
        assert!(
            largura > 0 && altura > 0 && self.fps > 0,
            "dimensões e fps precisam ser positivos"
        );
        let pixels_por_segundo = f64::from(largura) * f64::from(altura) * f64::from(self.fps);
        let bitrate = pixels_por_segundo * self.qualidade.bits_por_pixel();
        (bitrate as u32).clamp(BITRATE_MINIMO, BITRATE_MAXIMO)
    }

    /// Calcula quantos bytes reservar no anel para a retenção configurada, incluindo folga para picos.
    pub fn capacidade_anel(&self, bitrate_video: u32) -> u64 {
        let bytes_por_segundo = (u64::from(bitrate_video) + u64::from(BITRATE_AUDIO)) / 8;
        let segundos = u64::from(self.retencao_segundos) + MARGEM_RETENCAO_SEGUNDOS;
        (bytes_por_segundo as f64 * segundos as f64 * FOLGA_PICOS_VBR) as u64
    }

    /// Tempo retido no anel, em unidades de 100 ns.
    pub fn retencao_unidades(&self) -> i64 {
        (i64::from(self.retencao_segundos) + MARGEM_RETENCAO_SEGUNDOS as i64) * UNIDADES_POR_SEGUNDO
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn bitrate_cresce_com_a_resolucao_e_respeita_limites() {
        let config = Config::default();
        assert_eq!(config.bitrate_video(2560, 1440), 30_965_760);
        assert_eq!(config.bitrate_video(320, 240), BITRATE_MINIMO);
        assert_eq!(config.bitrate_video(15360, 8640), BITRATE_MAXIMO);
    }

    #[test]
    fn capacidade_cobre_a_retencao_com_folga() {
        let config = Config::default();
        let bitrate = 30_000_000;
        let minimo = (bitrate as u64 + BITRATE_AUDIO as u64) / 8 * 130;
        assert!(config.capacidade_anel(bitrate) > minimo);
    }

    #[test]
    fn resolucao_reduz_mantendo_proporcao_e_nunca_amplia() {
        assert_eq!(Resolucao::Nativa.aplicar(2560, 1440), (2560, 1440));
        assert_eq!(Resolucao::P1080.aplicar(2560, 1440), (1920, 1080));
        assert_eq!(Resolucao::P720.aplicar(3440, 1440), (1720, 720));
        assert_eq!(Resolucao::P1440.aplicar(1920, 1080), (1920, 1080));
        assert_eq!(Resolucao::Nativa.aplicar(1367, 769), (1366, 768));
    }

    #[test]
    fn apenas_ajustes_de_gravacao_reiniciam_a_captura() {
        let atual = Config::default();
        let mut so_duracao = atual.clone();
        so_duracao.duracao_clipe = 60;
        so_duracao.iniciar_com_windows = true;
        assert!(!atual.requer_reinicio(&so_duracao));

        let mut outro_fps = atual.clone();
        outro_fps.fps = 30;
        assert!(atual.requer_reinicio(&outro_fps));
    }

    #[test]
    fn campos_ausentes_usam_o_padrao() {
        let config: Config = serde_json::from_str(r#"{"fps":30}"#).unwrap();
        assert_eq!(config.fps, 30);
        assert_eq!(config.codec, Codec::H264);
    }
}
