//! Mensagens trocadas entre a interface e o motor, uma por linha em JSON sobre um pipe nomeado.

use crate::config::Config;
use serde::{Deserialize, Serialize};

/// Caminho do pipe nomeado em que o motor atende a interface.
pub const CAMINHO_PIPE: &str = r"\\.\pipe\zaga_motor";

/// Permite isolar ferramentas de diagnóstico sem conectar à sessão real do usuário.
pub fn caminho_pipe() -> String {
    std::env::var("ZAGA_PIPE")
        .map(|nome| format!(r"\\.\pipe\{nome}"))
        .unwrap_or_else(|_| CAMINHO_PIPE.to_owned())
}

/// Pedido enviado pela interface ao motor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pedido {
    ObterEstado,
    SalvarClipe { segundos: u32 },
    AplicarConfig(Config),
    SuspenderAtalhos(bool),
    Encerrar,
}

/// Resposta do motor a um [`Pedido`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resposta {
    Estado(EstadoMotor),
    Aceito,
    Falha(String),
}

/// Parâmetros efetivos da sessão de captura em andamento.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumoSessao {
    pub largura: u32,
    pub altura: u32,
    pub bitrate_video: u32,
    pub encoder: String,
    pub segundos_disponiveis: u32,
}

/// Fotografia do estado do motor, usada para desenhar a interface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstadoMotor {
    #[serde(default)]
    pub jogo_ativo: Option<String>,
    #[serde(default)]
    pub estado_autoclipagem: String,
    pub sessao: Option<ResumoSessao>,
    pub config: Config,
    pub pasta_clipes: String,
    pub pasta_dados: String,
    pub uso_cpu_decimos: u32,
    pub atalhos_indisponiveis: Vec<usize>,
    pub ultimo_erro: Option<String>,
}

/// Serializa uma mensagem como uma linha terminada em `\n`.
pub fn codificar_linha<T: Serialize>(mensagem: &T) -> serde_json::Result<String> {
    let mut linha = serde_json::to_string(mensagem)?;
    linha.push('\n');
    Ok(linha)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn pedido_sobrevive_a_ida_e_volta() {
        let pedido = Pedido::SalvarClipe { segundos: 60 };
        let linha = codificar_linha(&pedido).unwrap();
        assert!(linha.ends_with('\n'));
        assert_eq!(serde_json::from_str::<Pedido>(linha.trim_end()).unwrap(), pedido);
    }
}
