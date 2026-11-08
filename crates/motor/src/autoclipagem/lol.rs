use super::rede;
use anyhow::Result;
use reqwest::blocking::Client;
use serde_json::Value;
use zaga_nucleo::autoclipagem::EventoJogo;

#[derive(Default)]
pub(super) struct Lol {
    partida: String,
    ultimo_id: i64,
    inicio_ms: Option<i64>,
    jogador: String,
    mapa: String,
    ultima_kill: i64,
    sequencia: u32,
    grupo: u32,
}

impl Lol {
    pub fn consultar(&mut self, cliente: &Client, agora: i64) -> Result<Vec<EventoJogo>> {
        let dados = rede::json(cliente.get("https://127.0.0.1:2999/liveclientdata/allgamedata"))?;
        Ok(self.processar(&dados, agora))
    }

    pub fn processar(&mut self, dados: &Value, agora: i64) -> Vec<EventoJogo> {
        let tempo = dados["gameData"]["gameTime"].as_f64().unwrap_or(0.0);
        let inicio = agora - (tempo * 1000.0) as i64;
        if self.inicio_ms.is_none_or(|anterior| (anterior - inicio).abs() > 30_000) {
            *self = Self::default();
            self.inicio_ms = Some(inicio);
            self.partida = inicio.to_string();
            self.ultimo_id = -1;
            self.jogador = rede::texto(&dados["activePlayer"], "riotIdGameName");
            if self.jogador.is_empty() {
                self.jogador = rede::texto(&dados["activePlayer"], "summonerName");
            }
            self.mapa = rede::texto(&dados["gameData"], "mapName");
        }
        if self.jogador.is_empty() {
            return Vec::new();
        }
        let mut resultado = Vec::new();
        for evento in dados["events"]["Events"].as_array().into_iter().flatten() {
            let id = evento["EventID"].as_i64().unwrap_or(-1);
            if id <= self.ultimo_id {
                continue;
            }
            self.ultimo_id = id;
            let instante =
                self.inicio_ms.unwrap_or(inicio) + (evento["EventTime"].as_f64().unwrap_or(0.0) * 1000.0) as i64;
            if agora - instante > 30_000 {
                continue;
            }
            let tipo = rede::texto(evento, "EventName");
            let autor = rede::texto(evento, "KillerName");
            if autor != self.jogador {
                continue;
            }
            if tipo == "ChampionKill" {
                if instante - self.ultima_kill > 35_000 {
                    self.sequencia = 0;
                    self.grupo += 1;
                }
                self.sequencia += 1;
                self.ultima_kill = instante;
            } else if tipo.eq_ignore_ascii_case("Multikill") {
                self.sequencia = self.sequencia.max(evento["KillStreak"].as_u64().unwrap_or(0) as u32);
            } else {
                continue;
            }
            resultado.push(EventoJogo {
                jogo: "League of Legends".into(),
                partida: self.partida.clone(),
                rodada: self.grupo.to_string(),
                id: id.to_string(),
                instante_ms: instante,
                primeiro_ms: None,
                mapa: self.mapa.clone(),
                eliminacoes: self.sequencia,
                vitoria: false,
            });
        }
        resultado
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    #[test]
    fn filtra_outros_jogadores_e_nao_repete_historico() {
        let dados = json!({"activePlayer":{"riotIdGameName":"Ana"},"gameData":{"gameTime":100,"mapName":"Rift"},
            "events":{"Events":[{"EventID":0,"EventName":"ChampionKill","KillerName":"Outro","EventTime":98},
            {"EventID":1,"EventName":"ChampionKill","KillerName":"Ana","EventTime":99},
            {"EventID":2,"EventName":"Multikill","KillerName":"Ana","KillStreak":2,"EventTime":100}]}});
        let mut lol = Lol::default();
        let eventos = lol.processar(&dados, 200_000);
        assert_eq!(eventos.len(), 2);
        assert_eq!(eventos[1].eliminacoes, 2);
        assert!(lol.processar(&dados, 200_000).is_empty());
    }
}
