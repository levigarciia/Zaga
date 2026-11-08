mod cs2;
mod fortnite;
mod lol;
mod rede;
pub(crate) mod steam;
mod valorant;

use crate::controlador::Controlador;
use crate::detector_jogos::{self, JogoDetectado};
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};
use zaga_nucleo::autoclipagem::{Coordenador, EventoJogo};

pub(crate) fn iniciar(controlador: Arc<Controlador>) -> Result<()> {
    std::thread::Builder::new()
        .name("zaga_autoclipagem".into())
        .spawn(move || {
            if let Err(erro) = executar(&controlador) {
                controlador.informar_autoclipagem(format!("Integrações indisponíveis: {erro:#}"));
            }
        })?;
    Ok(())
}

fn executar(controlador: &Controlador) -> Result<()> {
    let local = rede::cliente(true)?;
    let remoto = rede::cliente(false)?;
    let token = token_cs2(controlador.pasta_dados())?;
    let (envio, recebimento) = mpsc::sync_channel::<EventoJogo>(256);
    let cs2_erro = cs2::iniciar(token.clone(), envio)
        .err()
        .map(|_| "A porta de integração do CS2 está ocupada");
    let mut instalado = None;
    let mut jogo: Option<JogoDetectado> = None;
    let mut ausente_desde: Option<Instant> = None;
    let mut coordenador = Coordenador::default();
    let mut lol = lol::Lol::default();
    let mut valorant = valorant::Valorant::default();
    let mut fortnite = fortnite::Fortnite::default();
    let mut steam = steam::Steam::default();
    let mut proxima_consulta = Instant::now();
    let mut proxima_retencao = Instant::now();
    let mut ultima_retencao = false;
    while !crate::encerrando() {
        let config = controlador.config();
        let detectado = detector_jogos::detectar(jogo.as_ref().map(|j| j.pid));
        if detectado.is_some() {
            ausente_desde = None;
            if jogo.as_ref().map(|j| j.pid) != detectado.as_ref().map(|j| j.pid) {
                coordenador.cancelar();
                lol = lol::Lol::default();
            }
            jogo = detectado;
        } else if jogo.as_ref().is_some_and(detector_jogos::continua_em_execucao) {
            ausente_desde = None;
        } else if jogo.is_some() {
            let inicio = *ausente_desde.get_or_insert_with(Instant::now);
            let espera = if jogo
                .as_ref()
                .is_some_and(|j| matches!(j.nome.as_str(), "VALORANT" | "Fortnite"))
            {
                Duration::from_secs(240)
            } else {
                Duration::from_secs(10)
            };
            if inicio.elapsed() >= espera {
                jogo = None;
            }
        }
        controlador.atualizar_jogo(jogo.clone());
        if !config.autoclipagem.ativa || !config.captura_ativa {
            steam.cancelar();
            coordenador.cancelar();
            if let Err(erro) = controlador.atualizar_retencao_partida(false) {
                crate::registro::registrar(format!("Não foi possível reduzir a retenção: {erro:#}"));
            }
            controlador.informar_autoclipagem("Autoclipagem desativada");
            for _ in recebimento.try_iter() {}
            std::thread::sleep(Duration::from_secs(1));
            continue;
        }
        let agora = crate::tempo::unix_ms();
        match steam.consultar(jogo.as_ref(), &config.autoclipagem, controlador.pasta_dados(), agora) {
            Ok(destaques) => {
                for destaque in destaques {
                    if let Err(erro) = controlador.salvar_destaque(destaque) {
                        controlador.informar_autoclipagem(format!("Conquista não salva: {erro:#}"));
                    }
                }
            }
            Err(erro) => {
                steam.estado = format!("Steam: {erro}");
            }
        }
        for evento in recebimento.try_iter() {
            if config.autoclipagem.habilitada(&evento.jogo) && jogo.as_ref().is_some_and(|j| j.nome == evento.jogo) {
                coordenador.registrar(evento, agora, &config.autoclipagem);
            }
        }
        if let Some(jogo) = &jogo {
            if config.autoclipagem.habilitada(&jogo.nome) && Instant::now() >= proxima_consulta {
                let consulta = match jogo.nome.as_str() {
                    "League of Legends" => lol.consultar(&local, agora),
                    "VALORANT" => valorant.consultar(&local, &remoto),
                    "Fortnite" => fortnite.consultar(&config.autoclipagem.conta_epic),
                    "Counter Strike 2" => {
                        if let Some(erro) = cs2_erro {
                            Err(anyhow::anyhow!(erro))
                        } else if instalado.as_ref() != Some(&jogo.executavel) {
                            cs2::instalar(&jogo.executavel, &token).map(|_| {
                                instalado = Some(jogo.executavel.clone());
                                Vec::new()
                            })
                        } else {
                            Ok(Vec::new())
                        }
                    }
                    _ => Ok(Vec::new()),
                };
                match consulta {
                    Ok(eventos) => {
                        for evento in eventos {
                            coordenador.registrar(evento, agora, &config.autoclipagem);
                        }
                        let mensagem = match jogo.nome.as_str() {
                            "VALORANT" => valorant.estado(),
                            "Fortnite" => fortnite.estado(),
                            "Counter Strike 2" => "Integração CS2 configurada. Reinicie o jogo se acabou de habilitar",
                            _ => "Acompanhando eventos da partida",
                        };
                        controlador.informar_autoclipagem(mensagem);
                    }
                    Err(erro) if jogo.nome == "Fortnite" => {
                        controlador.informar_autoclipagem(format!("Fortnite: {erro}"));
                    }
                    Err(_) => controlador.informar_autoclipagem(match jogo.nome.as_str() {
                        "League of Legends" => "LoL: aguardando os dados de uma partida em andamento",
                        "VALORANT" => {
                            "VALORANT: sessão ou detalhes ainda indisponíveis. A integração depende do cliente Riot"
                        }
                        "Fortnite" => "Fortnite: aguardando replay compatível e identidade do jogador nos logs",
                        _ => "CS2: não foi possível configurar a integração. Confira a pasta do jogo e a porta 12762",
                    }),
                }
                let intervalo = if matches!(jogo.nome.as_str(), "League of Legends" | "Fortnite") {
                    1
                } else {
                    10
                };
                proxima_consulta = Instant::now() + Duration::from_secs(intervalo);
            }
        } else {
            controlador.informar_autoclipagem("Aguardando um jogo compatível");
        }
        if config.autoclipagem.steam
            && jogo.as_ref().is_some_and(|j| !config.autoclipagem.habilitada(&j.nome))
            && !steam.estado.is_empty()
        {
            controlador.informar_autoclipagem(&steam.estado);
        }
        for destaque in coordenador.prontos(crate::tempo::unix_ms(), &config.autoclipagem) {
            if !config.autoclipagem.habilitada(&destaque.jogo) {
                continue;
            }
            if let Err(erro) = controlador.salvar_destaque(destaque) {
                controlador.informar_autoclipagem(format!("Destaque não salvo: {erro:#}"));
            }
        }
        let longa = [
            ("VALORANT", valorant.necessita_retencao()),
            ("Fortnite", fortnite.necessita_retencao()),
        ]
        .into_iter()
        .any(|(nome, em_partida)| {
            config.autoclipagem.habilitada(nome)
                && (coordenador.tem_pendentes(nome) || (em_partida && jogo.as_ref().is_some_and(|j| j.nome == nome)))
        });
        if longa != ultima_retencao || Instant::now() >= proxima_retencao {
            match controlador.atualizar_retencao_partida(longa) {
                Ok(()) => proxima_retencao = Instant::now() + Duration::from_secs(1),
                Err(erro) => {
                    controlador.informar_autoclipagem(format!("Não foi possível ajustar a retenção: {erro:#}"));
                    proxima_retencao = Instant::now() + Duration::from_secs(10);
                }
            }
            ultima_retencao = longa;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Ok(())
}

fn ler_cauda(caminho: &Path, maximo: u64) -> Result<String> {
    let mut arquivo = File::open(caminho).context("Log do jogo indisponível")?;
    let tamanho = arquivo.metadata()?.len();
    arquivo.seek(SeekFrom::Start(tamanho.saturating_sub(maximo)))?;
    let mut dados = Vec::new();
    arquivo.take(maximo).read_to_end(&mut dados)?;
    Ok(String::from_utf8_lossy(&dados).into_owned())
}

fn token_cs2(pasta: &Path) -> Result<String> {
    let caminho = pasta.join("cs2.token");
    match std::fs::read_to_string(&caminho) {
        Ok(token) if token.len() == 32 && token.bytes().all(|b| b.is_ascii_hexdigit()) => return Ok(token),
        Ok(_) => anyhow::bail!("O token local do CS2 está inválido"),
        Err(erro) if erro.kind() == std::io::ErrorKind::NotFound => {}
        Err(erro) => return Err(erro.into()),
    }
    let token = format!(
        "{:032x}",
        unsafe { windows::Win32::System::Com::CoCreateGuid() }?.to_u128()
    );
    std::fs::write(caminho, &token)?;
    Ok(token)
}
