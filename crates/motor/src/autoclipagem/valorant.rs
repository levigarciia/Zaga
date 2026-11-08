use super::rede;
use anyhow::{Context, Result, ensure};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::blocking::{Client, RequestBuilder};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use zaga_nucleo::autoclipagem::EventoJogo;

#[derive(Default)]
pub(super) struct Valorant {
    processadas: BTreeSet<String>,
    atual: Option<String>,
    encerrada: Option<(String, u32)>,
    detectada_ms: i64,
    ultimo_fim_ms: i64,
}

struct SessaoRiot {
    jogador: String,
    token: String,
    entitlement: String,
    regiao: String,
    shard: String,
    versao: String,
}

impl Valorant {
    pub(super) fn necessita_retencao(&self) -> bool {
        self.encerrada.is_some()
            || self
                .atual
                .as_ref()
                .is_some_and(|partida| !self.processadas.contains(partida))
    }

    pub(super) fn estado(&self) -> &'static str {
        if self.encerrada.is_some() {
            return "VALORANT: aguardando os detalhes da partida encerrada";
        }
        if self
            .atual
            .as_ref()
            .is_some_and(|partida| !self.processadas.contains(partida))
        {
            return "VALORANT: aguardando o fim da partida para gerar os destaques";
        }
        "VALORANT: aguardando uma nova partida"
    }

    pub fn consultar(&mut self, local: &Client, remoto: &Client) -> Result<Vec<EventoJogo>> {
        let sessao = autenticar(local)?;
        let resposta = sessao
            .pedido(
                remoto,
                &format!(
                    "https://glz-{}-1.{}.a.pvp.net/core-game/v1/players/{}",
                    sessao.regiao, sessao.shard, sessao.jogador
                ),
            )
            .send()
            .context("Não foi possível consultar a partida atual")?;
        let atual = if resposta.status() == reqwest::StatusCode::NOT_FOUND {
            None
        } else {
            let dados = rede::ler_json(resposta)?;
            Some(
                dados["MatchID"]
                    .as_str()
                    .context("Identificador de partida indisponível")?
                    .to_owned(),
            )
        };
        if self.atual != atual {
            if let Some(anterior) = self.atual.take() {
                if !self.processadas.contains(&anterior) {
                    self.encerrada = Some((anterior, 0));
                }
            }
            self.atual = atual;
            self.detectada_ms = crate::tempo::unix_ms();
        }
        let fim = fim_no_log().unwrap_or(0);
        if fim > self.detectada_ms && fim > self.ultimo_fim_ms {
            self.ultimo_fim_ms = fim;
            if let Some(partida) = &self.atual {
                if !self.processadas.contains(partida) {
                    self.encerrada = Some((partida.clone(), 0));
                }
            }
        }
        let Some((partida, tentativas)) = self.encerrada.clone() else {
            return Ok(Vec::new());
        };
        if tentativas >= 18 {
            self.encerrada = None;
            self.processadas.insert(partida);
            anyhow::bail!("Os detalhes da partida não ficaram disponíveis após três minutos");
        }
        self.encerrada = Some((partida.clone(), tentativas + 1));
        let dados = rede::json(sessao.pedido(
            remoto,
            &format!(
                "https://pd.{}.a.pvp.net/match-details/v1/matches/{partida}",
                sessao.shard
            ),
        ))?;
        let eventos = eventos_da_partida(&dados, &sessao.jogador, &partida)?;
        self.processadas.insert(partida);
        self.encerrada = None;
        Ok(eventos)
    }
}

fn fim_no_log() -> Option<i64> {
    let pasta = PathBuf::from(std::env::var_os("LOCALAPPDATA")?);
    let log = super::ler_cauda(&pasta.join("VALORANT/Saved/Logs/ShooterGame.log"), 2 * 1024 * 1024).ok()?;
    log.lines()
        .rev()
        .filter(|linha| linha.contains("Loopstate changed from INGAME"))
        .find_map(|linha| {
            let (_, resto) = linha.split_once('[')?;
            let (data, _) = resto.split_once(']')?;
            chrono::NaiveDateTime::parse_from_str(data, "%Y.%m.%d-%H.%M.%S:%3f")
                .ok()
                .map(|data| data.and_utc().timestamp_millis())
        })
}

impl SessaoRiot {
    fn pedido(&self, cliente: &Client, url: &str) -> RequestBuilder {
        let plataforma = STANDARD.encode(r#"{"platformType":"PC","platformOS":"Windows","platformOSVersion":"10.0.19045.1.256.64bit","platformChipset":"Unknown"}"#);
        let pedido = cliente
            .get(url)
            .bearer_auth(&self.token)
            .header("X-Riot-Entitlements-JWT", &self.entitlement)
            .header("X-Riot-ClientPlatform", plataforma);
        if self.versao.is_empty() {
            pedido
        } else {
            pedido.header("X-Riot-ClientVersion", &self.versao)
        }
    }
}

fn autenticar(cliente: &Client) -> Result<SessaoRiot> {
    let pasta = PathBuf::from(std::env::var_os("LOCALAPPDATA").context("Pasta de dados indisponível")?);
    let lockfile = std::fs::read_to_string(pasta.join("Riot Games/Riot Client/Config/lockfile"))
        .context("Cliente Riot ainda não está disponível")?;
    let partes: Vec<_> = lockfile.trim().split(':').collect();
    ensure!(
        partes.len() == 5 && partes[4] == "https",
        "Formato de sessão Riot incompatível"
    );
    let porta: u16 = partes[2].parse().context("Porta da sessão inválida")?;
    ensure!(porta > 0 && !partes[3].is_empty(), "Sessão Riot inválida");
    let endpoint = format!("https://127.0.0.1:{porta}");
    let tokens = rede::json(
        cliente
            .get(format!("{endpoint}/entitlements/v1/token"))
            .basic_auth("riot", Some(partes[3])),
    )?;
    let sessoes = rede::json(
        cliente
            .get(format!("{endpoint}/product-session/v1/external-sessions"))
            .basic_auth("riot", Some(partes[3])),
    )?;
    let jogo = sessoes
        .as_object()
        .and_then(|s| s.values().find(|s| s["productId"] == "valorant"))
        .context("Sessão do VALORANT não encontrada")?;
    let regiao = jogo["launchConfiguration"]["arguments"]
        .as_array()
        .and_then(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .find_map(|a| a.strip_prefix("-ares-deployment="))
        })
        .context("Região do VALORANT indisponível")?
        .to_owned();
    let shard = match regiao.as_str() {
        "br" | "latam" | "na" => "na",
        "eu" => "eu",
        "ap" | "sea" => "ap",
        "kr" => "kr",
        _ => anyhow::bail!("Região do VALORANT não reconhecida"),
    }
    .to_owned();
    let jogador = rede::texto(&tokens, "subject");
    let token = rede::texto(&tokens, "accessToken");
    let entitlement = rede::texto(&tokens, "token");
    ensure!(
        !jogador.is_empty() && !token.is_empty() && !entitlement.is_empty(),
        "Sessão Riot incompleta"
    );
    let versao = versao_do_cliente(&pasta).unwrap_or_else(|| rede::texto(&jogo["launchConfiguration"], "version"));
    Ok(SessaoRiot {
        jogador,
        token,
        entitlement,
        regiao,
        shard,
        versao,
    })
}

fn versao_do_cliente(pasta: &std::path::Path) -> Option<String> {
    let caminho = pasta.join("VALORANT/Saved/Logs/ShooterGame.log");
    let arquivo = std::fs::File::open(caminho).ok()?;
    let mut leitor = std::io::BufReader::new(std::io::Read::take(arquivo, 2 * 1024 * 1024));
    let mut linha = String::new();
    use std::io::BufRead;
    while leitor.read_line(&mut linha).ok()? > 0 {
        if let Some((_, versao)) = linha.split_once("CI server version:") {
            let versao = versao.trim();
            if !versao.is_empty() && versao.len() <= 256 {
                return Some(versao.to_owned());
            }
        }
        linha.clear();
    }
    None
}

pub(super) fn eventos_da_partida(dados: &Value, jogador: &str, partida: &str) -> Result<Vec<EventoJogo>> {
    ensure!(!jogador.is_empty() && !partida.is_empty(), "Partida sem jogador local");
    let inicio = dados["matchInfo"]["gameStartMillis"]
        .as_i64()
        .context("Partida sem referência de tempo")?;
    ensure!(inicio > 0, "Referência de tempo inválida");
    let mapa_original = rede::texto(&dados["matchInfo"], "mapId");
    let mapa = mapa_original.rsplit('/').next().unwrap_or(&mapa_original).to_owned();
    let mapa = match mapa.as_str() {
        "Ascent" => "Ascent",
        "Bonsai" => "Split",
        "Duality" => "Bind",
        "Triad" => "Haven",
        "Port" => "Icebox",
        "Foxtrot" => "Breeze",
        "Canyon" => "Fracture",
        "Pitt" => "Pearl",
        "Jam" => "Lotus",
        "Juliett" => "Sunset",
        "Infinity" => "Abyss",
        _ => &mapa,
    }
    .to_owned();
    let mut kills: Vec<_> = dados["kills"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|k| k["killer"].as_str() == Some(jogador) && k["victim"].as_str() != Some(jogador))
        .collect();
    kills.sort_by_key(|kill| kill["gameTime"].as_i64().unwrap_or(0));
    let mut rodadas = BTreeMap::<i64, (u32, i64, i64)>::new();
    for kill in kills {
        let tempo = kill["gameTime"].as_i64().context("Eliminação sem instante")?;
        let rodada = kill["round"].as_i64().context("Eliminação sem rodada")?;
        ensure!(tempo >= 0 && rodada >= 0, "Evento de partida inválido");
        let item = rodadas.entry(rodada).or_default();
        item.0 += 1;
        let instante = inicio.checked_add(tempo).context("Instante de eliminação inválido")?;
        item.1 = item.1.max(instante);
        if item.2 == 0 {
            item.2 = instante;
        }
    }
    Ok(rodadas
        .into_iter()
        .map(|(rodada, (eliminacoes, instante_ms, primeiro))| EventoJogo {
            jogo: "VALORANT".into(),
            partida: partida.into(),
            rodada: rodada.to_string(),
            id: format!("{partida}:{rodada}"),
            instante_ms,
            primeiro_ms: Some(primeiro),
            mapa: mapa.clone(),
            eliminacoes,
            vitoria: false,
        })
        .collect())
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    #[test]
    fn estado_distingue_menu_partida_e_processamento() {
        let mut valorant = Valorant::default();
        assert!(!valorant.necessita_retencao());
        assert_eq!(valorant.estado(), "VALORANT: aguardando uma nova partida");
        valorant.atual = Some("partida".into());
        assert!(valorant.necessita_retencao());
        assert_eq!(
            valorant.estado(),
            "VALORANT: aguardando o fim da partida para gerar os destaques"
        );
        valorant.encerrada = Some(("partida".into(), 0));
        assert!(valorant.necessita_retencao());
        assert_eq!(
            valorant.estado(),
            "VALORANT: aguardando os detalhes da partida encerrada"
        );
        valorant.encerrada = None;
        valorant.processadas.insert("partida".into());
        assert!(!valorant.necessita_retencao());
        assert_eq!(valorant.estado(), "VALORANT: aguardando uma nova partida");
        valorant.atual = None;
        assert_eq!(valorant.estado(), "VALORANT: aguardando uma nova partida");
    }

    #[test]
    fn agrupa_kills_do_jogador_por_rodada_com_horario_do_servidor() {
        let d = json!({"matchInfo":{"gameStartMillis":100000,"mapId":"/Game/Maps/Bonsai/Bonsai"},
            "kills":[{"killer":"eu","victim":"a","round":1,"gameTime":1000},
                {"killer":"eu","victim":"b","round":1,"gameTime":2000},
                {"killer":"outro","victim":"c","round":1,"gameTime":3000}]});
        let e = eventos_da_partida(&d, "eu", "partida").unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!((e[0].eliminacoes, e[0].instante_ms), (2, 102000));
        assert_eq!(e[0].mapa, "Split");
    }
}
