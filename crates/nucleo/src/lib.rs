//! Lógica pura do Zaga: buffer circular, seleção de clipes, configuração e protocolo.
//!
//! Use esta crate para tudo que não depende de APIs do Windows, de modo que possa ser testado sem GPU.

pub mod anel;
pub mod atalho;
pub mod cabecalho;
pub mod config;
pub mod jogos;
pub mod mistura;
pub mod protocolo;
pub mod relogio_audio;
pub mod ritmo;

/// Quantidade de unidades de 100 ns em um segundo, a base de tempo comum a vídeo e áudio.
pub const UNIDADES_POR_SEGUNDO: i64 = 10_000_000;
