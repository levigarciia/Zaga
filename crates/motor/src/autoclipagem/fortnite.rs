use aes::cipher::{BlockDecrypt, KeyInit, generic_array::GenericArray};
use anyhow::{Context, Result, ensure};
use chrono::{Datelike, Timelike};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::System::Time::{TIME_ZONE_INFORMATION, TzSpecificLocalTimeToSystemTime};
use zaga_nucleo::autoclipagem::EventoJogo;

#[derive(Default)]
pub(super) struct Fortnite {
    vistos: BTreeMap<PathBuf, (u64, SystemTime)>,
    observados: BTreeMap<PathBuf, ObservacaoReplay>,
    iniciados: BTreeSet<PathBuf>,
    jogador: String,
    inicializado: bool,
    em_partida: bool,
}

struct ObservacaoReplay {
    identidade: (u64, SystemTime),
    ultima_atividade: Option<Instant>,
}

impl Fortnite {
    pub(super) fn necessita_retencao(&self) -> bool {
        self.em_partida
    }

    pub(super) fn estado(&self) -> &'static str {
        if self.em_partida {
            return "Fortnite: acompanhando a partida e aguardando o replay completo";
        }
        "Fortnite: aguardando o início de uma partida com gravação de replay habilitada"
    }

    fn detectar_partida(&mut self, pasta: &Path) -> Result<()> {
        self.em_partida = false;
        if !pasta.exists() {
            self.observados.clear();
            return Ok(());
        }
        let mut presentes = BTreeSet::new();
        for entrada in std::fs::read_dir(pasta)?.flatten() {
            let caminho = entrada.path();
            if caminho.extension().is_none_or(|ext| ext != "replay") {
                continue;
            }
            let metadata = entrada.metadata()?;
            let modificado = metadata.modified()?;
            presentes.insert(caminho.clone());
            let identidade = (metadata.len(), modificado);
            let observacao = self
                .observados
                .entry(caminho.clone())
                .or_insert_with(|| ObservacaoReplay {
                    identidade,
                    ultima_atividade: modificado
                        .elapsed()
                        .ok()
                        .filter(|idade| *idade <= Duration::from_secs(120))
                        .and_then(|idade| Instant::now().checked_sub(idade)),
                });
            if observacao.identidade != identidade {
                observacao.identidade = identidade;
                observacao.ultima_atividade = Some(Instant::now());
            }
            let recente = observacao
                .ultima_atividade
                .is_some_and(|atividade| atividade.elapsed() <= Duration::from_secs(120));
            let cabecalho = File::open(&caminho)
                .ok()
                .and_then(|mut arquivo| ler_cabecalho(&mut arquivo).ok());
            if recente && cabecalho.is_some_and(|cabecalho| cabecalho.ao_vivo) {
                self.iniciados.insert(caminho.clone());
                self.em_partida = true;
            }
            if self.iniciados.contains(&caminho)
                && self.vistos.get(&caminho) != Some(&(metadata.len(), modificado))
                && recente
            {
                self.em_partida = true;
            }
        }
        self.observados.retain(|caminho, _| presentes.contains(caminho));
        Ok(())
    }

    pub fn consultar(&mut self, conta: &str) -> Result<Vec<EventoJogo>> {
        if !conta.is_empty() {
            self.jogador = conta.to_lowercase();
        }
        let local = PathBuf::from(std::env::var_os("LOCALAPPDATA").context("Pasta local ausente")?);
        let pasta = local.join("FortniteGame/Saved/Demos");
        self.detectar_partida(&pasta)?;
        if self.jogador.is_empty() {
            let log = super::ler_cauda(&local.join("FortniteGame/Saved/Logs/FortniteGame.log"), 2 * 1024 * 1024)?;
            let regex =
                regex::Regex::new(r#"(?i)(?:EpicAccountId|LocalUserId|LoggedInAccountId)[\s:=\"']+([a-f0-9]{32})"#)?;
            if let Some(id) = regex.captures(&log).and_then(|c| c.get(1)) {
                self.jogador = id.as_str().to_lowercase();
            }
        }
        let mut resultado = Vec::new();
        for entrada in std::fs::read_dir(pasta)
            .context("Fortnite: aguardando arquivos de replay")?
            .flatten()
        {
            let caminho = entrada.path();
            if caminho.extension().is_none_or(|ext| ext != "replay") {
                continue;
            }
            let metadata = entrada.metadata()?;
            let identidade = (metadata.len(), metadata.modified()?);
            if self.vistos.get(&caminho) == Some(&identidade) {
                continue;
            }
            let cabecalho = ler_cabecalho(&mut File::open(&caminho)?)?;
            if cabecalho.ao_vivo {
                continue;
            }
            if !self.inicializado && !self.iniciados.contains(&caminho) {
                self.vistos.insert(caminho, identidade);
                continue;
            }
            ensure!(
                !self.jogador.is_empty(),
                "Fortnite: identidade Epic não encontrada no log do jogo"
            );
            let eventos = ler_eventos(&caminho, &self.jogador)?;
            self.vistos.insert(caminho, identidade);
            resultado.extend(eventos);
        }
        self.inicializado = true;
        Ok(resultado)
    }
}

struct Cabecalho {
    ao_vivo: bool,
    inicio_ms: i64,
    duracao_ms: u32,
    chave: Vec<u8>,
    cifrado: bool,
}

fn ler_cabecalho(leitor: &mut impl Read) -> Result<Cabecalho> {
    ensure!(u32_le(leitor)? == 0x1ca2e27f, "Replay Fortnite com assinatura inválida");
    let versao = u32_le(leitor)?;
    ensure!(
        (3..=7).contains(&versao),
        "Versão de replay Fortnite ainda não suportada"
    );
    if versao >= 7 {
        let quantidade = u32_le(leitor)?;
        ensure!(quantidade <= 1024, "Versões customizadas excedem o limite");
        descartar(leitor, quantidade as usize * 20)?;
    }
    let duracao_ms = u32_le(leitor)?;
    descartar(leitor, 8)?;
    let _nome = string_unreal(leitor)?;
    let ao_vivo = u32_le(leitor)? != 0;
    let mut ticks = [0u8; 8];
    leitor.read_exact(&mut ticks)?;
    let inicio_ms = inicio_replay_utc(u64::from_le_bytes(ticks), None)?;
    descartar(leitor, 4)?;
    let cifrado = versao >= 6 && u32_le(leitor)? != 0;
    let chave = if versao >= 6 {
        let tamanho = u32_le(leitor)?;
        ensure!(tamanho <= 32, "Chave de replay inválida");
        let mut chave = vec![0; tamanho as usize];
        leitor.read_exact(&mut chave)?;
        chave
    } else {
        Vec::new()
    };
    Ok(Cabecalho {
        ao_vivo,
        inicio_ms,
        duracao_ms,
        chave,
        cifrado,
    })
}

fn inicio_replay_utc(ticks: u64, fuso: Option<&TIME_ZONE_INFORMATION>) -> Result<i64> {
    let ticks = ticks & 0x3fff_ffff_ffff_ffff;
    ensure!(ticks > 621_355_968_000_000_000, "Replay sem referência de tempo válida");
    let milissegundos = (ticks - 621_355_968_000_000_000) / 10_000;
    let data = chrono::DateTime::from_timestamp_millis(milissegundos as i64)
        .context("Data do replay fora do intervalo suportado")?;
    let local = SYSTEMTIME {
        wYear: u16::try_from(data.year()).context("Ano do replay inválido")?,
        wMonth: data.month() as u16,
        wDay: data.day() as u16,
        wHour: data.hour() as u16,
        wMinute: data.minute() as u16,
        wSecond: data.second() as u16,
        wMilliseconds: data.timestamp_subsec_millis() as u16,
        ..Default::default()
    };
    let mut utc = SYSTEMTIME::default();
    unsafe { TzSpecificLocalTimeToSystemTime(fuso.map(std::ptr::from_ref), &local, &mut utc) }
        .context("Não foi possível converter o horário local do replay para UTC")?;
    let data = chrono::NaiveDate::from_ymd_opt(utc.wYear.into(), utc.wMonth.into(), utc.wDay.into())
        .and_then(|data| {
            data.and_hms_milli_opt(
                utc.wHour.into(),
                utc.wMinute.into(),
                utc.wSecond.into(),
                utc.wMilliseconds.into(),
            )
        })
        .context("Horário UTC do replay inválido")?;
    Ok(data.and_utc().timestamp_millis())
}

pub(super) fn ler_eventos(caminho: &Path, jogador: &str) -> Result<Vec<EventoJogo>> {
    ensure!(
        jogador.len() == 32 && jogador.bytes().all(|c| c.is_ascii_hexdigit()),
        "Conta Epic inválida"
    );
    let mut arquivo = File::open(caminho)?;
    let tamanho = arquivo.metadata()?.len();
    let cabecalho = ler_cabecalho(&mut arquivo)?;
    ensure!(!cabecalho.ao_vivo, "Replay ainda está sendo gravado");
    let partida = format!("replay:{}", cabecalho.inicio_ms);
    let mut engine = 0;
    let mut kills = Vec::new();
    let mut vitoria = false;
    let mut ultima_eliminacao = None;
    let mut instante_colocacao = None;
    while arquivo.stream_position()?.saturating_add(8) <= tamanho {
        let tipo = u32_le(&mut arquivo)?;
        let bytes = u32_le(&mut arquivo)? as u64;
        let fim = arquivo
            .stream_position()?
            .checked_add(bytes)
            .context("Chunk inválido")?;
        ensure!(fim <= tamanho, "Replay incompleto");
        if tipo == 0 {
            ensure!(bytes <= 4 * 1024 * 1024, "Cabeçalho de rede excede o limite");
            let mut dados = vec![0u8; bytes as usize];
            arquivo.read_exact(&mut dados)?;
            let mut leitor = &dados[..];
            ensure!(u32_le(&mut leitor)? == 0x2cf5a13d, "Cabeçalho de rede inválido");
            let rede = u32_le(&mut leitor)?;
            if rede >= 19 {
                let quantidade = u32_le(&mut leitor)?;
                ensure!(quantidade <= 1024, "Cabeçalho de rede excede o limite");
                descartar(&mut leitor, quantidade as usize * 20)?;
            }
            descartar(&mut leitor, 4)?;
            engine = u32_le(&mut leitor)?;
        } else if tipo == 3 {
            ensure!(bytes <= 4 * 1024 * 1024, "Evento de replay excede o limite");
            let mut dados = vec![0u8; bytes as usize];
            arquivo.read_exact(&mut dados)?;
            let mut leitor = &dados[..];
            let _id = string_unreal(&mut leitor)?;
            let grupo = string_unreal(&mut leitor)?;
            let metadata = string_unreal(&mut leitor)?;
            let instante = u32_le(&mut leitor)?;
            descartar(&mut leitor, 4)?;
            let comprimento = u32_le(&mut leitor)? as usize;
            ensure!(comprimento <= leitor.len(), "Evento de replay truncado");
            let payload = decifrar(&leitor[..comprimento], &cabecalho)?;
            if grupo == "playerElim" {
                ensure!(engine > 0, "Replay sem versão do motor");
                if let Some(autor) = autor_eliminacao(&payload, engine)? {
                    ultima_eliminacao = Some(ultima_eliminacao.unwrap_or(0).max(instante));
                    if autor.eq_ignore_ascii_case(jogador) {
                        kills.push(instante);
                    }
                }
            } else if metadata == "AthenaMatchTeamStats" {
                let mut p = &payload[..];
                descartar(&mut p, 4)?;
                vitoria = u32_le(&mut p)? == 1;
                instante_colocacao = Some(instante);
            }
        }
        arquivo.seek(SeekFrom::Start(fim))?;
    }
    kills.sort_unstable();
    let mut grupos = BTreeMap::<u32, (u32, u32, u32)>::new();
    let mut grupo = 0;
    let mut anterior = None;
    for instante in kills {
        if anterior.is_some_and(|antigo| instante.saturating_sub(antigo) > 15_000) {
            grupo += 1;
        }
        let valor = grupos.entry(grupo).or_default();
        if valor.0 == 0 {
            valor.2 = instante;
        }
        valor.0 += 1;
        valor.1 = instante;
        anterior = Some(instante);
    }
    let mut eventos: Vec<_> = grupos
        .into_iter()
        .map(|(grupo, (eliminacoes, instante, primeiro))| EventoJogo {
            jogo: "Fortnite".into(),
            partida: partida.clone(),
            rodada: grupo.to_string(),
            id: format!("{partida}:{grupo}"),
            instante_ms: cabecalho.inicio_ms + i64::from(instante),
            primeiro_ms: Some(cabecalho.inicio_ms + i64::from(primeiro)),
            mapa: "Fortnite".into(),
            eliminacoes,
            vitoria: false,
        })
        .collect();
    if vitoria {
        eventos.push(EventoJogo {
            jogo: "Fortnite".into(),
            partida: partida.clone(),
            rodada: "vitória".into(),
            id: format!("{partida}:vitória"),
            instante_ms: cabecalho.inicio_ms
                + i64::from(ultima_eliminacao.or(instante_colocacao).unwrap_or(cabecalho.duracao_ms)),
            primeiro_ms: None,
            mapa: "Fortnite".into(),
            eliminacoes: 0,
            vitoria: true,
        });
    }
    Ok(eventos)
}

fn autor_eliminacao(dados: &[u8], engine: u32) -> Result<Option<String>> {
    let mut leitor = dados;
    let versao = u32_le(&mut leitor)?;
    ensure!(
        matches!(versao, 3..=6 | 10),
        "Versão de eliminação Fortnite ainda não suportada: {versao}"
    );
    descartar(&mut leitor, if versao >= 6 { 81 } else { 41 })?;
    if engine >= 34 {
        descartar(&mut leitor, 80)?;
    }
    let _vitima = jogador_replay(&mut leitor, versao)?;
    let autor = jogador_replay(&mut leitor, versao)?;
    descartar(&mut leitor, 1)?;
    let derrubado = u32_le(&mut leitor)?;
    ensure!(derrubado <= 1, "Indicador de derrubada Fortnite inválido");
    Ok((derrubado == 0).then_some(autor))
}

fn jogador_replay(leitor: &mut &[u8], versao: u32) -> Result<String> {
    if versao < 6 {
        return string_unreal(leitor);
    }
    let mut tipo = [0];
    leitor.read_exact(&mut tipo)?;
    match tipo[0] {
        3 => Ok(String::new()),
        0x10 => string_unreal(leitor),
        0x11 => {
            let mut tamanho = [0];
            leitor.read_exact(&mut tamanho)?;
            ensure!(tamanho[0] == 16, "Identificador de jogador incompatível");
            let mut id = [0; 16];
            leitor.read_exact(&mut id)?;
            Ok(id.iter().map(|byte| format!("{byte:02x}")).collect())
        }
        _ => anyhow::bail!("Tipo de jogador Fortnite incompatível"),
    }
}

fn decifrar(dados: &[u8], cabecalho: &Cabecalho) -> Result<Vec<u8>> {
    if !cabecalho.cifrado {
        return Ok(dados.to_vec());
    }
    ensure!(
        cabecalho.chave.len() == 32 && !dados.is_empty() && dados.len() % 16 == 0,
        "Replay cifrado ainda não contém uma chave válida"
    );
    let aes = aes::Aes256::new_from_slice(&cabecalho.chave).map_err(|_| anyhow::anyhow!("Chave inválida"))?;
    let mut saida = dados.to_vec();
    for bloco in saida.chunks_exact_mut(16) {
        aes.decrypt_block(GenericArray::from_mut_slice(bloco));
    }
    let preenchimento = usize::from(*saida.last().context("Evento cifrado vazio")?);
    ensure!(
        (1..=16).contains(&preenchimento)
            && saida[saida.len() - preenchimento..]
                .iter()
                .all(|byte| usize::from(*byte) == preenchimento),
        "Evento de replay não pôde ser decifrado"
    );
    saida.truncate(saida.len() - preenchimento);
    Ok(saida)
}

fn u32_le(leitor: &mut impl Read) -> Result<u32> {
    let mut bytes = [0; 4];
    leitor.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn descartar(leitor: &mut impl Read, tamanho: usize) -> Result<()> {
    let lidos = std::io::copy(&mut leitor.take(tamanho as u64), &mut std::io::sink())?;
    ensure!(lidos == tamanho as u64, "Replay truncado");
    Ok(())
}

fn string_unreal(leitor: &mut impl Read) -> Result<String> {
    let tamanho = u32_le(leitor)? as i32;
    ensure!(tamanho.unsigned_abs() <= 32_768, "Texto de replay excede o limite");
    let bytes = if tamanho < 0 {
        tamanho.unsigned_abs() * 2
    } else {
        tamanho as u32
    };
    let mut dados = vec![0; bytes as usize];
    leitor.read_exact(&mut dados)?;
    let texto = if tamanho < 0 {
        String::from_utf16(
            &dados
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        )?
    } else {
        String::from_utf8(dados)?
    };
    Ok(texto.trim_end_matches('\0').to_owned())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn converte_horario_local_do_replay_sem_descartar_destaque_recente() {
        let local_ms = 1_794_010_224_168u64;
        let ticks = local_ms * 10_000 + 621_355_968_000_000_000;
        let fuso = TIME_ZONE_INFORMATION {
            Bias: 240,
            ..Default::default()
        };
        let inicio = inicio_replay_utc(ticks, Some(&fuso)).unwrap();
        assert_eq!(inicio, local_ms as i64 + 4 * 60 * 60 * 1000);
        assert_eq!(
            inicio_replay_utc(ticks, Some(&TIME_ZONE_INFORMATION::default())).unwrap(),
            local_ms as i64
        );
        assert!(inicio_replay_utc(0, Some(&fuso)).is_err());
        let config = zaga_nucleo::autoclipagem::ConfigAutoclipagem::default();
        let mut coordenador = zaga_nucleo::autoclipagem::Coordenador::default();
        let agora = inicio + 1_057_637;
        assert!(coordenador.registrar(
            EventoJogo {
                jogo: "Fortnite".into(),
                partida: "regressao".into(),
                rodada: "0".into(),
                id: "dupla".into(),
                instante_ms: inicio + 155_390,
                primeiro_ms: Some(inicio + 155_317),
                mapa: "Fortnite".into(),
                eliminacoes: 2,
                vitoria: false,
            },
            agora,
            &config
        ));
        let destaques = coordenador.prontos(agora + 5_000, &config);
        assert_eq!(destaques.len(), 1);
        assert_eq!(destaques[0].eliminacoes, 2);
    }

    fn replay_ativo(caminho: &Path, ao_vivo: bool) {
        let mut bytes = Vec::new();
        for valor in [0x1ca2e27fu32, 3, 0, 0, 0, 1] {
            bytes.extend(valor.to_le_bytes());
        }
        bytes.push(0);
        bytes.extend(u32::from(ao_vivo).to_le_bytes());
        bytes.extend(638_000_000_000_000_000u64.to_le_bytes());
        bytes.extend(0u32.to_le_bytes());
        std::fs::write(caminho, bytes).unwrap();
    }

    #[test]
    fn retencao_acompanha_replay_recente_ate_processamento() {
        let pasta = std::env::temp_dir().join(format!("zaga_estado_fortnite_{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let caminho = pasta.join("partida.replay");
        let mut fortnite = Fortnite::default();
        fortnite.detectar_partida(&pasta).unwrap();
        assert!(!fortnite.necessita_retencao());
        replay_ativo(&caminho, true);
        fortnite.detectar_partida(&pasta).unwrap();
        assert!(fortnite.necessita_retencao());
        replay_ativo(&caminho, false);
        fortnite.detectar_partida(&pasta).unwrap();
        assert!(fortnite.necessita_retencao());
        let metadata = std::fs::metadata(&caminho).unwrap();
        fortnite
            .vistos
            .insert(caminho.clone(), (metadata.len(), metadata.modified().unwrap()));
        fortnite.detectar_partida(&pasta).unwrap();
        assert!(!fortnite.necessita_retencao());
        replay_ativo(&caminho, true);
        let arquivo = std::fs::OpenOptions::new().write(true).open(&caminho).unwrap();
        arquivo
            .set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(180))
            .unwrap();
        drop(arquivo);
        let mut fortnite = Fortnite::default();
        fortnite.detectar_partida(&pasta).unwrap();
        assert!(!fortnite.necessita_retencao());
        let modificado = std::fs::metadata(&caminho).unwrap().modified().unwrap();
        let mut arquivo = std::fs::OpenOptions::new().append(true).open(&caminho).unwrap();
        std::io::Write::write_all(&mut arquivo, &[1, 2, 3]).unwrap();
        arquivo.set_modified(modificado).unwrap();
        drop(arquivo);
        fortnite.detectar_partida(&pasta).unwrap();
        assert!(fortnite.necessita_retencao());
        fortnite.observados.get_mut(&caminho).unwrap().ultima_atividade =
            Some(Instant::now() - Duration::from_secs(180));
        fortnite.detectar_partida(&pasta).unwrap();
        assert!(!fortnite.necessita_retencao());
        std::fs::remove_dir_all(pasta).unwrap();
    }

    #[test]
    fn rejeita_tamanho_malicioso_e_replay_truncado() {
        assert!(string_unreal(&mut &i32::MIN.to_le_bytes()[..]).is_err());
        assert!(ler_cabecalho(&mut &[0u8; 8][..]).is_err());
    }
    #[test]
    fn conta_eliminacao_completa_sem_confundir_derrubado() {
        let mut dados = 6u32.to_le_bytes().to_vec();
        dados.extend([0; 81 + 80]);
        dados.push(3);
        dados.extend([0x11, 16]);
        dados.extend([0xab; 16]);
        dados.push(0);
        dados.extend(0u32.to_le_bytes());
        assert_eq!(autor_eliminacao(&dados, 34).unwrap(), Some("ab".repeat(16)));
        let tamanho = dados.len();
        dados[tamanho - 4] = 1;
        assert_eq!(autor_eliminacao(&dados, 34).unwrap(), None);
    }

    #[test]
    fn le_eliminacao_versao_dez_e_rejeita_payload_invalido() {
        let mut dados = 10u32.to_le_bytes().to_vec();
        dados.extend([0; 161]);
        dados.extend([0x11, 16]);
        dados.extend([0xcd; 16]);
        dados.extend([0x11, 16]);
        dados.extend([0xab; 16]);
        dados.push(0);
        dados.extend(0u32.to_le_bytes());
        assert_eq!(autor_eliminacao(&dados, 34).unwrap(), Some("ab".repeat(16)));
        let tamanho = dados.len();
        dados[tamanho - 4] = 1;
        assert_eq!(autor_eliminacao(&dados, 34).unwrap(), None);
        dados[tamanho - 4] = 2;
        assert!(autor_eliminacao(&dados, 34).is_err());
        assert!(autor_eliminacao(&dados[..tamanho - 1], 34).is_err());
        dados[..4].copy_from_slice(&11u32.to_le_bytes());
        assert!(autor_eliminacao(&dados, 34).unwrap_err().to_string().contains("11"));
    }

    #[test]
    fn le_replay_cifrado_e_agrupa_apenas_eliminacoes_do_jogador_local() {
        use aes::cipher::BlockEncrypt;
        fn texto(saida: &mut Vec<u8>, valor: &str) {
            saida.extend(((valor.len() + 1) as u32).to_le_bytes());
            saida.extend(valor.as_bytes());
            saida.push(0);
        }
        fn chunk(saida: &mut Vec<u8>, tipo: u32, dados: &[u8]) {
            saida.extend(tipo.to_le_bytes());
            saida.extend((dados.len() as u32).to_le_bytes());
            saida.extend(dados);
        }
        let mut bytes = Vec::new();
        for n in [0x1ca2e27f_u32, 6, 60_000, 0, 0] {
            bytes.extend(n.to_le_bytes());
        }
        texto(&mut bytes, "Teste sintético");
        bytes.extend(0u32.to_le_bytes());
        let inicio = 1_700_000_000_000i64;
        bytes.extend(((inicio as u64 * 10_000) + 621_355_968_000_000_000).to_le_bytes());
        for n in [0u32, 1, 32] {
            bytes.extend(n.to_le_bytes());
        }
        bytes.extend([7; 32]);
        let mut rede = Vec::new();
        for n in [0x2cf5a13du32, 18, 0, 34, 0] {
            rede.extend(n.to_le_bytes());
        }
        chunk(&mut bytes, 0, &rede);
        let sem_eliminacoes = bytes.clone();
        let aes = aes::Aes256::new_from_slice(&[7; 32]).unwrap();
        for (id, instante, autor, derrubado) in [
            ("a", 1000u32, 0xab, 0u32),
            ("b", 3000, 0xab, 0),
            ("colega", 30_000, 0xef, 0),
            ("derrubada", 35_000, 0xef, 1),
        ] {
            let mut dados = 6u32.to_le_bytes().to_vec();
            dados.extend([0; 161]);
            dados.extend([3, 0x11, 16]);
            dados.extend([autor; 16]);
            dados.push(0);
            dados.extend(derrubado.to_le_bytes());
            let padding = 16 - dados.len() % 16;
            dados.extend(std::iter::repeat_n(padding as u8, padding));
            for bloco in dados.chunks_exact_mut(16) {
                aes.encrypt_block(GenericArray::from_mut_slice(bloco));
            }
            let mut evento = Vec::new();
            texto(&mut evento, id);
            texto(&mut evento, "playerElim");
            texto(&mut evento, "");
            evento.extend(instante.to_le_bytes());
            evento.extend(instante.to_le_bytes());
            evento.extend((dados.len() as u32).to_le_bytes());
            evento.extend(dados);
            chunk(&mut bytes, 3, &evento);
        }
        let caminho = std::env::temp_dir().join(format!("zaga_replay_cifrado_{}.replay", std::process::id()));
        std::fs::write(&caminho, bytes).unwrap();
        let eventos = ler_eventos(&caminho, &"ab".repeat(16)).unwrap();
        assert_eq!(eventos.len(), 1);
        assert_eq!(eventos[0].eliminacoes, 2);
        let inicio_utc = inicio_replay_utc(inicio as u64 * 10_000 + 621_355_968_000_000_000, None).unwrap();
        assert_eq!(eventos[0].instante_ms, inicio_utc + 3000);
        assert_eq!(eventos[0].primeiro_ms, Some(inicio_utc + 1000));
        assert!(ler_eventos(&caminho, &"cd".repeat(16)).unwrap().is_empty());
        let mut dados = Vec::new();
        dados.extend(0u32.to_le_bytes());
        dados.extend(1u32.to_le_bytes());
        dados.extend([8; 8]);
        for bloco in dados.chunks_exact_mut(16) {
            aes.encrypt_block(GenericArray::from_mut_slice(bloco));
        }
        let mut evento = Vec::new();
        texto(&mut evento, "colocacao");
        texto(&mut evento, "stats");
        texto(&mut evento, "AthenaMatchTeamStats");
        evento.extend(50_000u32.to_le_bytes());
        evento.extend(50_000u32.to_le_bytes());
        evento.extend((dados.len() as u32).to_le_bytes());
        evento.extend(dados);
        let mut bytes = std::fs::read(&caminho).unwrap();
        chunk(&mut bytes, 3, &evento);
        std::fs::write(&caminho, bytes).unwrap();
        let eventos = ler_eventos(&caminho, &"ab".repeat(16)).unwrap();
        assert_eq!(eventos.len(), 2);
        let vitoria = eventos.iter().find(|evento| evento.vitoria).unwrap();
        assert_eq!(vitoria.instante_ms, inicio_utc + 30_000);
        assert_eq!(vitoria.eliminacoes, 0);
        let config = zaga_nucleo::autoclipagem::ConfigAutoclipagem::default();
        let mut coordenador = zaga_nucleo::autoclipagem::Coordenador::default();
        assert!(coordenador.registrar(vitoria.clone(), inicio_utc + 60_000, &config));
        let destaques = coordenador.prontos(inicio_utc + 65_000, &config);
        assert_eq!(destaques.len(), 1);
        assert_eq!(destaques[0].inicio_ms, inicio_utc + 10_000);
        assert_eq!(destaques[0].fim_ms, inicio_utc + 35_000);
        let eventos = ler_eventos(&caminho, &"cd".repeat(16)).unwrap();
        assert_eq!(eventos.len(), 1);
        assert!(eventos[0].vitoria);
        assert_eq!(eventos[0].instante_ms, inicio_utc + 30_000);
        let mut bytes = sem_eliminacoes;
        chunk(&mut bytes, 3, &evento);
        std::fs::write(&caminho, bytes).unwrap();
        let eventos = ler_eventos(&caminho, &"ab".repeat(16)).unwrap();
        assert_eq!(eventos.len(), 1);
        assert_eq!(eventos[0].instante_ms, inicio_utc + 50_000);
        std::fs::remove_file(caminho).unwrap();
    }
}
