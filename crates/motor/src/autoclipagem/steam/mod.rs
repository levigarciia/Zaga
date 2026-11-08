pub(crate) mod manifesto;
mod sdk;

use crate::detector_jogos::JogoDetectado;
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};
use zaga_nucleo::autoclipagem::{ConfigAutoclipagem, Destaque};

const BIBLIOTECA: &[u8] = include_bytes!("../../../../../assets/steam/steam_api64.dll");

pub(crate) fn executar_monitor() -> bool {
    let argumentos: Vec<_> = std::env::args().collect();
    if argumentos.get(1).is_none_or(|a| a != "--monitor-steam") {
        return false;
    }
    let resultado = (|| -> Result<()> {
        ensure!(argumentos.len() == 3, "Argumentos do monitor Steam inválidos");
        let caminho = Path::new(&argumentos[2]);
        ensure!(std::fs::read(caminho)? == BIBLIOTECA, "Biblioteca Steam incompatível");
        let encerrado = Arc::new(AtomicBool::new(false));
        let sinal = encerrado.clone();
        std::thread::spawn(move || {
            let _ = std::io::stdin().read(&mut [0u8; 1]);
            sinal.store(true, Ordering::Release);
            std::process::exit(0);
        });
        let sdk = sdk::Sdk::abrir(caminho)?;
        while !encerrado.load(Ordering::Acquire) {
            let dados = sdk.consultar()?;
            println!("{}", json!({"conquistas": dados}));
            std::io::stdout().flush()?;
            std::thread::sleep(Duration::from_secs(3));
        }
        Ok(())
    })();
    if resultado.is_err() {
        println!(
            "{}",
            json!({"erro": "Steam indisponível para acompanhar conquistas deste jogo"})
        );
    }
    true
}

struct Monitor {
    processo: Child,
    mensagens: mpsc::Receiver<Value>,
    rastreador: Rastreador,
    ultimo_sinal: Instant,
}

impl Drop for Monitor {
    fn drop(&mut self) {
        let _ = self.processo.kill();
        let _ = self.processo.wait();
    }
}

#[derive(Default)]
struct Rastreador {
    conhecidos: BTreeMap<String, bool>,
    vistos: BTreeSet<String>,
}

impl Rastreador {
    fn atualizar(&mut self, dados: &[Value], limite: u32, agora: i64) -> Vec<(String, String, f64, i64)> {
        let mut novos = Vec::new();
        for conquista in dados {
            let Some(id) = conquista["id"].as_str().filter(|id| !id.is_empty() && id.len() <= 256) else {
                continue;
            };
            let Some(desbloqueada) = conquista["desbloqueada"].as_bool() else {
                continue;
            };
            let anterior = self.conhecidos.insert(id.to_owned(), desbloqueada);
            if anterior != Some(false) || !desbloqueada || self.vistos.contains(id) {
                continue;
            }
            let Some(percentual) = conquista["percentual"].as_f64() else {
                self.conhecidos.insert(id.to_owned(), false);
                continue;
            };
            if !(0.0..=100.0).contains(&percentual) || percentual >= f64::from(limite) {
                self.vistos.insert(id.to_owned());
                continue;
            }
            let Some(instante) = conquista["instante"].as_i64().and_then(|s| s.checked_mul(1000)) else {
                continue;
            };
            if instante <= 0 || instante > agora + 10_000 || agora.saturating_sub(instante) > 15_000 {
                self.vistos.insert(id.to_owned());
                continue;
            }
            self.vistos.insert(id.to_owned());
            let nome = conquista["nome"]
                .as_str()
                .filter(|n| !n.trim().is_empty())
                .unwrap_or(id);
            novos.push((id.to_owned(), nome.chars().take(160).collect(), percentual, instante));
        }
        novos
    }
}

#[derive(Default)]
pub(super) struct Steam {
    alvo: Option<u32>,
    monitor: Option<Monitor>,
    pendentes: Vec<Destaque>,
    proxima_tentativa: Option<Instant>,
    pub(super) estado: String,
}

impl Steam {
    pub(super) fn consultar(
        &mut self,
        jogo: Option<&JogoDetectado>,
        config: &ConfigAutoclipagem,
        pasta: &Path,
        agora: i64,
    ) -> Result<Vec<Destaque>> {
        let identidade = jogo
            .filter(|_| config.ativa && config.steam)
            .and_then(|j| manifesto::identificar(&j.executavel).map(|(app, _)| (app, j)));
        if self.alvo != identidade.map(|(app, _)| app) {
            self.alvo = identidade.map(|(app, _)| app);
            self.monitor = None;
            self.pendentes.clear();
            self.proxima_tentativa = None;
        }
        if !config.ativa || !config.steam {
            self.pendentes.clear();
            return Ok(Vec::new());
        }
        let Some((app, jogo)) = identidade else {
            return Ok(Vec::new());
        };
        if self.monitor.is_none() && self.proxima_tentativa.is_none_or(|p| Instant::now() >= p) {
            self.proxima_tentativa = Some(Instant::now() + Duration::from_secs(30));
            self.monitor = Some(iniciar(app, pasta)?);
            self.estado = "Steam: aguardando estatísticas das conquistas".into();
        }
        let Some(monitor) = self.monitor.as_mut() else {
            return Ok(self.prontos(agora));
        };
        for mensagem in monitor.mensagens.try_iter() {
            monitor.ultimo_sinal = Instant::now();
            if mensagem["erro"].is_string() {
                self.estado = "Steam: cliente indisponível ou jogo sem acesso às conquistas".into();
            }
            let Some(dados) = mensagem["conquistas"].as_array().filter(|d| d.len() <= 20_000) else {
                continue;
            };
            self.estado = if dados.is_empty() {
                "Steam: aguardando conquistas disponíveis"
            } else {
                "Steam: acompanhando conquistas raras"
            }
            .into();
            for (id, nome, percentual, instante) in monitor.rastreador.atualizar(dados, config.raridade_steam, agora) {
                if self.pendentes.len() >= 512 {
                    break;
                }
                self.pendentes.push(Destaque {
                    titulo: format!("Conquista: {nome} ({percentual:.1}%)"),
                    jogo: jogo.nome.clone(),
                    partida: format!("steam:{app}"),
                    rodada: id,
                    inicio_ms: instante - i64::from(config.anteriores) * 1000,
                    fim_ms: instante + i64::from(config.posteriores) * 1000,
                    eliminacoes: 0,
                    mapa: String::new(),
                    vitoria: false,
                });
            }
        }
        if monitor.processo.try_wait()?.is_some() || monitor.ultimo_sinal.elapsed() > Duration::from_secs(20) {
            self.monitor = None;
            self.proxima_tentativa = Some(Instant::now() + Duration::from_secs(30));
            self.estado = "Steam: monitor indisponível, aguardando nova tentativa".into();
        }
        Ok(self.prontos(agora))
    }

    fn prontos(&mut self, agora: i64) -> Vec<Destaque> {
        let (prontos, aguardando) = self.pendentes.drain(..).partition(|d| agora >= d.fim_ms);
        self.pendentes = aguardando;
        prontos
    }

    pub(super) fn cancelar(&mut self) {
        self.alvo = None;
        self.monitor = None;
        self.pendentes.clear();
    }
}

fn iniciar(app: u32, pasta: &Path) -> Result<Monitor> {
    let pasta = pasta.join("steam_sdk_eb17909a");
    std::fs::create_dir_all(&pasta)?;
    let biblioteca = pasta.join("steam_api64.dll");
    if !biblioteca.exists() {
        let mut arquivo = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&biblioteca)?;
        arquivo.write_all(BIBLIOTECA)?;
    }
    ensure!(
        std::fs::read(&biblioteca)? == BIBLIOTECA,
        "Biblioteca Steam local incompatível"
    );
    let mut processo = Command::new(std::env::current_exe()?)
        .arg("--monitor-steam")
        .arg(biblioteca)
        .env("SteamAppId", app.to_string())
        .env("SteamGameId", app.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(0x08000000)
        .spawn()
        .context("Não foi possível iniciar o monitor Steam")?;
    let saida = processo.stdout.take().context("Monitor Steam sem canal de saída")?;
    let (envio, mensagens) = mpsc::sync_channel(8);
    std::thread::spawn(move || {
        let mut leitor = BufReader::new(saida);
        loop {
            let mut linha = String::new();
            if leitor
                .by_ref()
                .take(4 * 1024 * 1024 + 1)
                .read_line(&mut linha)
                .unwrap_or(0)
                == 0
                || linha.len() > 4 * 1024 * 1024
            {
                break;
            }
            if let Ok(dados) = serde_json::from_str(&linha) {
                if envio.send(dados).is_err() {
                    break;
                }
            }
        }
    });
    Ok(Monitor {
        processo,
        mensagens,
        rastreador: Rastreador::default(),
        ultimo_sinal: Instant::now(),
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn aguarda_contexto_posterior_e_cancela_pendentes_ao_desativar() {
        let destaque = Destaque {
            titulo: "Conquista: Teste (1.0%)".into(),
            jogo: "Portal 2".into(),
            partida: "steam:620".into(),
            rodada: "a".into(),
            inicio_ms: 80_000,
            fim_ms: 105_000,
            eliminacoes: 0,
            mapa: String::new(),
            vitoria: false,
        };
        let mut steam = Steam::default();
        steam.pendentes.push(destaque.clone());
        assert!(steam.prontos(104_999).is_empty());
        assert_eq!(steam.prontos(105_000), vec![destaque.clone()]);
        assert!(steam.prontos(106_000).is_empty());
        steam.pendentes.push(destaque);
        steam.cancelar();
        assert!(steam.prontos(106_000).is_empty());
    }

    #[test]
    fn biblioteca_distribuida_exporta_as_interfaces_utilizadas() {
        let caminho = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/steam/steam_api64.dll");
        let nomes = [
            "SteamAPI_InitFlat",
            "SteamAPI_IsSteamRunning",
            "SteamAPI_Shutdown",
            "SteamAPI_RunCallbacks",
            "SteamAPI_SteamUserStats_v013",
            "SteamAPI_ISteamUserStats_RequestGlobalAchievementPercentages",
            "SteamAPI_ISteamUserStats_GetNumAchievements",
            "SteamAPI_ISteamUserStats_GetAchievementName",
            "SteamAPI_ISteamUserStats_GetAchievementAndUnlockTime",
            "SteamAPI_ISteamUserStats_GetAchievementAchievedPercent",
            "SteamAPI_ISteamUserStats_GetAchievementDisplayAttribute",
        ];
        unsafe {
            let biblioteca = libloading::Library::new(caminho).unwrap();
            for nome in nomes {
                assert!(
                    biblioteca.get::<*const ()>(nome.as_bytes()).is_ok(),
                    "Exportação ausente: {nome}"
                );
            }
        }
    }

    #[test]
    fn ignora_historico_comuns_e_duplicatas_e_aceita_transicao_rara() {
        let mut rastreador = Rastreador::default();
        let bloqueada = json!({"id":"a", "nome":"Conquista", "desbloqueada":false});
        let rara = json!({"id":"a", "nome":"Conquista", "desbloqueada":true, "percentual":4.2, "instante":100});
        assert!(rastreador.atualizar(&[rara.clone()], 10, 100_000).is_empty());
        rastreador = Rastreador::default();
        rastreador.atualizar(&[bloqueada.clone()], 10, 100_000);
        let mut comum = rara.clone();
        comum["percentual"] = json!(10);
        assert!(rastreador.atualizar(&[comum], 10, 100_000).is_empty());
        rastreador = Rastreador::default();
        rastreador.atualizar(&[bloqueada], 10, 100_000);
        assert_eq!(rastreador.atualizar(&[rara.clone()], 10, 103_000).len(), 1);
        assert!(rastreador.atualizar(&[rara], 10, 104_000).is_empty());
    }

    #[test]
    fn aguarda_percentual_e_rejeita_desbloqueio_antigo() {
        let mut rastreador = Rastreador::default();
        let mut conquista = json!({"id":"a", "desbloqueada":false, "instante":100});
        rastreador.atualizar(&[conquista.clone()], 10, 100_000);
        conquista["desbloqueada"] = json!(true);
        assert!(rastreador.atualizar(&[conquista.clone()], 10, 101_000).is_empty());
        conquista["percentual"] = json!(1.0);
        assert_eq!(rastreador.atualizar(&[conquista.clone()], 10, 102_000).len(), 1);
        let mut outro = Rastreador::default();
        conquista["desbloqueada"] = json!(false);
        outro.atualizar(&[conquista.clone()], 10, 100_000);
        conquista["desbloqueada"] = json!(true);
        assert!(outro.atualizar(&[conquista], 10, 120_000).is_empty());
    }
}
