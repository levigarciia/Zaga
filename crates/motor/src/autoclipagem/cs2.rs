use super::rede;
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::mpsc::SyncSender;
use std::time::Duration;
use zaga_nucleo::autoclipagem::EventoJogo;

pub(super) const PORTA: u16 = 12762;

#[derive(Default)]
struct Rodada {
    partida: String,
    numero: i64,
    mapa: String,
    kills: u32,
    contador: u32,
    ultimo_ms: i64,
    primeiro_ms: Option<i64>,
    emitida: bool,
}

impl Rodada {
    fn processar(&mut self, dados: &Value, agora: i64) -> Vec<EventoJogo> {
        if dados["provider"]["steamid"].as_str().is_none() || dados["provider"]["steamid"] != dados["player"]["steamid"]
        {
            return Vec::new();
        }
        let mapa = rede::texto(&dados["map"], "name");
        let numero = dados["map"]["round"].as_i64().unwrap_or(-1);
        if mapa.is_empty() || numero < 0 || dados["map"]["phase"] == "warmup" {
            return Vec::new();
        }
        let contador = dados["player"]["match_stats"]["kills"].as_u64().unwrap_or(0) as u32;
        let mut eventos = Vec::new();
        if self.mapa != mapa || self.numero != numero {
            if let Some(evento) = self.concluir() {
                eventos.push(evento);
            }
            let partida = if self.mapa != mapa || numero < self.numero || self.partida.is_empty() {
                format!("{mapa}:{agora}")
            } else {
                self.partida.clone()
            };
            *self = Self {
                partida,
                numero,
                mapa: mapa.clone(),
                contador,
                ..Self::default()
            };
        }
        let kills_rodada = dados["player"]["state"]["round_kills"].as_u64().map(|n| n as u32);
        let kills = kills_rodada.unwrap_or_else(|| self.kills.saturating_add(contador.saturating_sub(self.contador)));
        self.contador = contador;
        if kills > self.kills {
            self.primeiro_ms.get_or_insert(agora);
            self.kills = kills;
            self.ultimo_ms = agora;
        }
        if dados["round"]["phase"] == "over" {
            if let Some(evento) = self.concluir() {
                eventos.push(evento);
            }
        }
        eventos
    }

    fn concluir(&mut self) -> Option<EventoJogo> {
        if self.kills == 0 || self.emitida || self.ultimo_ms == 0 {
            return None;
        }
        self.emitida = true;
        let mapa = self
            .mapa
            .rsplit('/')
            .next()
            .unwrap_or(&self.mapa)
            .trim_start_matches("de_")
            .trim_start_matches("cs_");
        let mapa = match mapa {
            "dust2" => "Dust II".to_owned(),
            _ => {
                let mut letras = mapa.chars();
                letras
                    .next()
                    .map(|p| p.to_uppercase().collect::<String>() + letras.as_str())
                    .unwrap_or_default()
            }
        };
        Some(EventoJogo {
            jogo: "Counter Strike 2".into(),
            partida: self.partida.clone(),
            rodada: self.numero.to_string(),
            id: format!("rodada:{}", self.numero),
            instante_ms: self.ultimo_ms,
            primeiro_ms: self.primeiro_ms,
            mapa,
            eliminacoes: self.kills,
            vitoria: false,
        })
    }
}

pub(super) fn instalar(executavel: &Path, token: &str) -> Result<()> {
    let game = executavel
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .context("Pasta do CS2 não reconhecida")?;
    let pasta = game.join("csgo/cfg");
    ensure!(pasta.is_dir(), "Pasta de configuração do CS2 não encontrada");
    let config = format!(
        r#""Zaga"
{{
    "uri" "http://127.0.0.1:{PORTA}"
    "timeout" "2.0"
    "buffer" "0.1"
    "throttle" "0.1"
    "auth" {{ "token" "{token}" }}
    "data" {{ "provider" "1" "map" "1" "round" "1" "player_id" "1" "player_state" "1" "player_match_stats" "1" }}
}}
"#
    );
    let destino = pasta.join("gamestate_integration_zaga.cfg");
    if destino.exists() {
        ensure!(
            std::fs::read_to_string(&destino)?.starts_with("\"Zaga\""),
            "A configuração existente do Zaga foi personalizada e não será substituída"
        );
    }
    std::fs::write(destino, config)?;
    Ok(())
}

pub(super) fn iniciar(token: String, envio: SyncSender<EventoJogo>) -> Result<()> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, PORTA))?;
    listener.set_nonblocking(true)?;
    std::thread::Builder::new().name("zaga_cs2".into()).spawn(move || {
        let mut rodada = Rodada::default();
        while !crate::encerrando() {
            match listener.accept() {
                Ok((mut conexao, _)) => {
                    let resultado = ler_pedido(&mut conexao).and_then(|dados| {
                        ensure!(dados["auth"]["token"].as_str() == Some(&token), "Origem não autorizada");
                        Ok(dados)
                    });
                    let status = if resultado.is_ok() { "200 OK" } else { "400 Bad Request" };
                    let _ = write!(
                        conexao,
                        "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                    if let Ok(dados) = resultado {
                        for evento in rodada.processar(&dados, crate::tempo::unix_ms()) {
                            let _ = envio.try_send(evento);
                        }
                    }
                }
                Err(_) => std::thread::sleep(Duration::from_millis(100)),
            }
        }
    })?;
    Ok(())
}

fn ler_pedido(conexao: &mut TcpStream) -> Result<Value> {
    conexao.set_read_timeout(Some(Duration::from_secs(1)))?;
    conexao.set_write_timeout(Some(Duration::from_secs(1)))?;
    let mut dados = Vec::new();
    let mut bloco = [0u8; 4096];
    loop {
        let lidos = conexao.read(&mut bloco)?;
        ensure!(lidos > 0, "Pedido incompleto");
        dados.extend_from_slice(&bloco[..lidos]);
        ensure!(dados.len() <= 64 * 1024, "Pedido excede o limite");
        if let Some(fim) = dados.windows(4).position(|b| b == b"\r\n\r\n") {
            let cabecalho = std::str::from_utf8(&dados[..fim])?;
            ensure!(cabecalho.starts_with("POST "), "Método inválido");
            let tamanho = cabecalho
                .lines()
                .find_map(|linha| {
                    let (nome, valor) = linha.split_once(':')?;
                    nome.eq_ignore_ascii_case("content-length")
                        .then(|| valor.trim().parse::<usize>().ok())
                        .flatten()
                })
                .context("Tamanho ausente")?;
            ensure!(tamanho <= 60 * 1024, "Conteúdo excede o limite");
            if dados.len() >= fim + 4 + tamanho {
                return Ok(serde_json::from_slice(&dados[fim + 4..fim + 4 + tamanho])?);
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;
    #[test]
    fn entrega_uma_vez_no_fim_da_rodada_e_ignora_espectador() {
        let mut rodada = Rodada::default();
        let mut d = json!({"provider":{"steamid":"1"},"player":{"steamid":"1","state":{"round_kills":4}},
            "map":{"name":"de_ancient","round":2},"round":{"phase":"live"}});
        assert!(rodada.processar(&d, 100_000).is_empty());
        d["round"]["phase"] = json!("over");
        let eventos = rodada.processar(&d, 101_000);
        assert_eq!(eventos[0].eliminacoes, 4);
        assert_eq!(eventos[0].mapa, "Ancient");
        assert!(rodada.processar(&d, 102_000).is_empty());
        d["player"]["steamid"] = json!("2");
        assert!(rodada.processar(&d, 103_000).is_empty());
    }
}
