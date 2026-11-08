//! Eventos normalizados e agrupamento de destaques, sem dependência do jogo ou do gravador.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Preferências da geração de clipes por eventos de gameplay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigAutoclipagem {
    pub steam: bool,
    pub raridade_steam: u32,
    pub conta_epic: String,
    pub ativa: bool,
    pub lol: bool,
    pub cs2: bool,
    pub valorant: bool,
    pub fortnite: bool,
    pub somente_jogos: bool,
    pub anteriores: u32,
    pub posteriores: u32,
    pub minimo_eliminacoes: u32,
    pub retencao_minutos: u32,
    pub limite_gib: u32,
}

impl Default for ConfigAutoclipagem {
    fn default() -> Self {
        Self {
            steam: true,
            raridade_steam: 10,
            conta_epic: String::new(),
            ativa: true,
            lol: true,
            cs2: true,
            valorant: true,
            fortnite: true,
            somente_jogos: false,
            anteriores: 20,
            posteriores: 5,
            minimo_eliminacoes: 2,
            retencao_minutos: 45,
            limite_gib: 16,
        }
    }
}

impl ConfigAutoclipagem {
    /// Verifica as fronteiras antes de reservar espaço ou iniciar integrações.
    pub fn validar(&self) -> Result<(), &'static str> {
        if !(1..=100).contains(&self.raridade_steam) {
            return Err("O limite de raridade Steam deve ficar entre 1 e 100 por cento");
        }
        if !self.conta_epic.is_empty()
            && (self.conta_epic.len() != 32 || !self.conta_epic.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err("O ID da conta Epic deve conter 32 caracteres hexadecimais");
        }
        if !(1..=120).contains(&self.anteriores) || !(1..=30).contains(&self.posteriores) {
            return Err("O contexto do clipe deve ficar entre 1 e 120 segundos antes e 1 e 30 depois");
        }
        if !(1..=5).contains(&self.minimo_eliminacoes) {
            return Err("A quantidade mínima de eliminações deve ficar entre 1 e 5");
        }
        if !(5..=180).contains(&self.retencao_minutos) || !(1..=128).contains(&self.limite_gib) {
            return Err("A retenção deve ficar entre 5 e 180 minutos e o limite entre 1 e 128 GiB");
        }
        Ok(())
    }

    /// Consulta se o adaptador de um jogo foi habilitado.
    pub fn habilitada(&self, jogo: &str) -> bool {
        self.ativa
            && match jogo {
                "League of Legends" => self.lol,
                "Counter Strike 2" => self.cs2,
                "VALORANT" => self.valorant,
                "Fortnite" => self.fortnite,
                _ => false,
            }
    }
}

/// Evento de um jogador local com identidade estável e instante UTC em milissegundos.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventoJogo {
    pub jogo: String,
    pub partida: String,
    pub rodada: String,
    pub id: String,
    pub instante_ms: i64,
    #[serde(default)]
    pub primeiro_ms: Option<i64>,
    pub mapa: String,
    pub eliminacoes: u32,
    pub vitoria: bool,
}

/// Intervalo final e metadados usados tanto na exportação quanto no título da biblioteca.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Destaque {
    pub titulo: String,
    pub jogo: String,
    pub partida: String,
    pub rodada: String,
    pub inicio_ms: i64,
    pub fim_ms: i64,
    pub eliminacoes: u32,
    pub mapa: String,
    pub vitoria: bool,
}

impl Destaque {
    fn atualizar_titulo(&mut self) {
        let evento = if self.vitoria {
            "Vitória".to_owned()
        } else if self.eliminacoes >= 5 && matches!(self.jogo.as_str(), "VALORANT" | "Counter Strike 2") {
            "Ace".to_owned()
        } else {
            format!("{}K", self.eliminacoes)
        };
        let contexto = if self.mapa.trim().is_empty() {
            &self.jogo
        } else {
            &self.mapa
        };
        self.titulo = format!("{evento}: {contexto}");
    }

    /// Produz um nome de arquivo válido sem alterar o título exibido ao usuário.
    pub fn nome_arquivo(&self) -> String {
        self.titulo
            .chars()
            .filter(|c| !c.is_control() && !"\\/:*?\"<>|".contains(*c))
            .take(100)
            .collect::<String>()
            .trim()
            .trim_end_matches('.')
            .to_owned()
    }
}

struct Candidato {
    destaque: Destaque,
    recebido_ms: i64,
    ultimo_evento_ms: i64,
}

/// Mantém uma sequência por rodada e ignora reenvios antes de exportar o maior destaque.
#[derive(Default)]
pub struct Coordenador {
    vistos: BTreeSet<(String, String, String)>,
    ordem_vistos: VecDeque<(String, String, String)>,
    pendentes: BTreeMap<(String, String, String), Candidato>,
}

impl Coordenador {
    /// Registra um evento e retorna falso para entradas inválidas, antigas ou repetidas.
    pub fn registrar(&mut self, evento: EventoJogo, agora_ms: i64, config: &ConfigAutoclipagem) -> bool {
        if !config.habilitada(&evento.jogo)
            || evento.partida.len() > 128
            || evento.rodada.len() > 128
            || evento.id.len() > 256
            || evento.mapa.len() > 256
            || evento.partida.is_empty()
            || evento.id.is_empty()
            || evento.instante_ms <= 0
            || evento.instante_ms > agora_ms + 10_000
            || evento.primeiro_ms.is_some_and(|p| p <= 0 || p > evento.instante_ms)
            || agora_ms.saturating_sub(evento.instante_ms) > i64::from(config.retencao_minutos) * 60_000
        {
            return false;
        }
        let identidade = (evento.jogo.clone(), evento.partida.clone(), evento.id.clone());
        if !self.vistos.insert(identidade.clone()) {
            return false;
        }
        self.ordem_vistos.push_back(identidade);
        if self.ordem_vistos.len() > 20_000 {
            if let Some(antigo) = self.ordem_vistos.pop_front() {
                self.vistos.remove(&antigo);
            }
        }
        let chave = (evento.jogo.clone(), evento.partida.clone(), evento.rodada.clone());
        if self.pendentes.len() >= 512 && !self.pendentes.contains_key(&chave) {
            return false;
        }
        let candidato = self.pendentes.entry(chave).or_insert_with(|| Candidato {
            destaque: Destaque {
                titulo: String::new(),
                jogo: evento.jogo,
                partida: evento.partida,
                rodada: evento.rodada,
                inicio_ms: evento.primeiro_ms.unwrap_or(evento.instante_ms) - i64::from(config.anteriores) * 1000,
                fim_ms: evento.instante_ms + i64::from(config.posteriores) * 1000,
                eliminacoes: 0,
                mapa: evento.mapa.clone(),
                vitoria: false,
            },
            recebido_ms: agora_ms,
            ultimo_evento_ms: evento.instante_ms,
        });
        candidato.recebido_ms = agora_ms;
        candidato.ultimo_evento_ms = candidato.ultimo_evento_ms.max(evento.instante_ms);
        candidato.destaque.eliminacoes = candidato.destaque.eliminacoes.max(evento.eliminacoes);
        candidato.destaque.vitoria |= evento.vitoria;
        candidato.destaque.inicio_ms = candidato
            .destaque
            .inicio_ms
            .min(evento.primeiro_ms.unwrap_or(evento.instante_ms) - i64::from(config.anteriores) * 1000);
        candidato.destaque.fim_ms = candidato
            .destaque
            .fim_ms
            .max(evento.instante_ms + i64::from(config.posteriores) * 1000);
        if !evento.mapa.is_empty() {
            candidato.destaque.mapa = evento.mapa;
        }
        candidato.destaque.atualizar_titulo();
        true
    }

    /// Retira sequências concluídas, conservando o contexto posterior e uma janela para promoção.
    pub fn prontos(&mut self, agora_ms: i64, config: &ConfigAutoclipagem) -> Vec<Destaque> {
        let chaves: Vec<_> = self
            .pendentes
            .iter()
            .filter(|(_, candidato)| {
                let espera = if candidato.destaque.jogo == "League of Legends" {
                    config.posteriores.max(35)
                } else {
                    config.posteriores
                };
                agora_ms >= candidato.destaque.fim_ms && agora_ms - candidato.recebido_ms >= i64::from(espera) * 1000
            })
            .map(|(chave, _)| chave.clone())
            .collect();
        chaves
            .into_iter()
            .filter_map(|chave| self.pendentes.remove(&chave))
            .map(|candidato| candidato.destaque)
            .filter(|d| config.habilitada(&d.jogo) && (d.vitoria || d.eliminacoes >= config.minimo_eliminacoes))
            .collect()
    }

    /// Consulte antes de reduzir a retenção usada pelos destaques que ainda aguardam contexto.
    pub fn tem_pendentes(&self, jogo: &str) -> bool {
        self.pendentes.values().any(|candidato| candidato.destaque.jogo == jogo)
    }

    /// Cancela candidatos ao desativar o recurso sem alterar a captura manual.
    pub fn cancelar(&mut self) {
        self.pendentes.clear();
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn configuracao_antiga_recebe_steam_e_valida_raridade() {
        let mut config: ConfigAutoclipagem = serde_json::from_str("{}").unwrap();
        assert!(config.steam);
        assert_eq!(config.raridade_steam, 10);
        assert!(config.validar().is_ok());
        for limite in [0, 101, u32::MAX] {
            config.raridade_steam = limite;
            assert!(config.validar().is_err());
        }
    }

    fn evento(id: &str, kills: u32, instante_ms: i64) -> EventoJogo {
        EventoJogo {
            jogo: "Counter Strike 2".into(),
            partida: "p1".into(),
            rodada: "3".into(),
            id: id.into(),
            instante_ms,
            primeiro_ms: None,
            mapa: "Inferno".into(),
            eliminacoes: kills,
            vitoria: false,
        }
    }

    #[test]
    fn pendencias_preservam_retencao_ate_o_contexto_posterior() {
        let mut coordenador = Coordenador::default();
        let config = ConfigAutoclipagem::default();
        let mut evento = evento("fortnite", 2, 100_000);
        evento.jogo = "Fortnite".into();
        coordenador.registrar(evento, 100_000, &config);
        assert!(coordenador.tem_pendentes("Fortnite"));
        assert!(!coordenador.tem_pendentes("VALORANT"));
        assert!(coordenador.prontos(104_000, &config).is_empty());
        assert!(coordenador.tem_pendentes("Fortnite"));
        assert_eq!(coordenador.prontos(106_000, &config).len(), 1);
        assert!(!coordenador.tem_pendentes("Fortnite"));
    }

    #[test]
    fn promove_sequencia_e_descarta_duplicatas() {
        let mut c = Coordenador::default();
        let cfg = ConfigAutoclipagem::default();
        assert!(c.registrar(evento("a", 2, 100_000), 100_000, &cfg));
        assert!(!c.registrar(evento("a", 2, 100_000), 101_000, &cfg));
        assert!(c.registrar(evento("b", 4, 102_000), 102_000, &cfg));
        assert!(c.prontos(105_000, &cfg).is_empty());
        let d = c.prontos(108_000, &cfg);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].titulo, "4K: Inferno");
        assert_eq!((d[0].inicio_ms, d[0].fim_ms), (80_000, 107_000));
        assert_eq!(d[0].nome_arquivo(), "4K Inferno");
    }

    #[test]
    fn separa_rodadas_e_rejeita_futuro_e_retencao_vencida() {
        let mut c = Coordenador::default();
        let cfg = ConfigAutoclipagem::default();
        assert!(!c.registrar(evento("f", 2, 500_000), 100_000, &cfg));
        assert!(!c.registrar(evento("v", 2, 100), 4_000_000, &cfg));
        c.registrar(evento("a", 2, 100_000), 100_000, &cfg);
        let mut outro = evento("b", 5, 101_000);
        outro.rodada = "4".into();
        c.registrar(outro, 101_000, &cfg);
        let d = c.prontos(110_000, &cfg);
        assert_eq!(d.len(), 2);
        assert_eq!(d[1].titulo, "Ace: Inferno");
    }
}
