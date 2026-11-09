use crate::services::miniaturas;
use crate::services::sistema::{DataLocal, data_local};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use zaga_nucleo::UNIDADES_POR_SEGUNDO;

const MESES: [&str; 12] = [
    "jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clipe {
    pub titulo: String,
    pub jogo: Option<String>,
    pub automatico: bool,
    pub caminho: PathBuf,
    pub bytes: u64,
    pub modificado: SystemTime,
    pub miniatura: Option<PathBuf>,
    pub duracao: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrdemClipes {
    Recentes,
    Antigos,
    Maiores,
}

pub fn listar_clipes(pasta: &Path, cache: &Path, pasta_dados: &Path) -> Vec<Clipe> {
    let Ok(entradas) = std::fs::read_dir(pasta) else {
        return Vec::new();
    };
    let mut clipes: Vec<Clipe> = entradas
        .flatten()
        .filter_map(|entrada| {
            let caminho = entrada.path();
            if !eh_clipe_concluido(&caminho) {
                return None;
            }
            let nome = caminho.file_stem()?.to_str()?;
            let destaque = zaga_nucleo::metadados::ler(pasta_dados, &caminho);
            let automatico = destaque.is_some();
            let jogo = identificar_jogo(destaque.as_ref().map(|d| d.jogo.as_str()), nome);
            let titulo = destaque
                .map(|d| d.titulo)
                .unwrap_or_else(|| titulo_do_arquivo(nome).to_owned());
            let metadados = entrada.metadata().ok()?;
            let modificado = metadados.modified().ok()?;
            let chave = miniaturas::chave(&caminho, metadados.len(), modificado);
            let (miniatura, duracao) = miniaturas::ler_cache(cache, &chave);
            Some(Clipe {
                titulo,
                jogo,
                automatico,
                caminho,
                bytes: metadados.len(),
                modificado,
                miniatura,
                duracao,
            })
        })
        .collect();
    ordenar(&mut clipes, OrdemClipes::Recentes);
    clipes
}

fn identificar_jogo(metadado: Option<&str>, nome: &str) -> Option<String> {
    let candidato = metadado.unwrap_or_else(|| titulo_do_arquivo(nome));
    zaga_nucleo::jogos::nomes_suportados()
        .find(|jogo| jogo.eq_ignore_ascii_case(candidato))
        .map(str::to_owned)
}

pub fn jogos_com_clipes(clipes: &[Clipe]) -> Vec<String> {
    clipes
        .iter()
        .filter_map(|clipe| clipe.jogo.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn ultimos_clipes(clipes: &[Clipe], jogo: Option<&str>, limite: usize) -> Vec<Clipe> {
    let mut resultado: Vec<Clipe> = clipes
        .iter()
        .filter(|clipe| jogo.is_none_or(|jogo| clipe.jogo.as_deref() == Some(jogo)))
        .cloned()
        .collect();
    ordenar(&mut resultado, OrdemClipes::Recentes);
    resultado.truncate(limite);
    resultado
}

pub fn filtrar_clipes(clipes: &[Clipe], busca: &str, ordem: OrdemClipes) -> Vec<Clipe> {
    let termo = busca.trim().to_lowercase();
    let mut resultado: Vec<Clipe> = clipes
        .iter()
        .filter(|clipe| clipe.titulo.to_lowercase().contains(&termo))
        .cloned()
        .collect();
    ordenar(&mut resultado, ordem);
    resultado
}

pub fn titulo_do_arquivo(nome: &str) -> &str {
    nome.char_indices()
        .filter(|(_, caractere)| *caractere == ' ')
        .find(|(posicao, _)| comeca_com_carimbo(&nome[posicao + 1..]))
        .map_or(nome, |(posicao, _)| &nome[..posicao])
}

pub fn formatar_tamanho(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    let megas = bytes as f64 / MIB;
    if megas >= 1024.0 {
        format!("{:.1} GB", megas / 1024.0).replace('.', ",")
    } else {
        format!("{megas:.0} MB")
    }
}

pub fn formatar_duracao(unidades: i64) -> String {
    let segundos = unidades.max(0) / UNIDADES_POR_SEGUNDO;
    format!("{}:{:02}", segundos / 60, segundos % 60)
}

pub fn rotulo_data(modificado: SystemTime) -> String {
    match (data_local(modificado), data_local(SystemTime::now())) {
        (Some(data), Some(hoje)) => formatar_data(data, hoje),
        _ => String::new(),
    }
}

fn formatar_data(data: DataLocal, hoje: DataLocal) -> String {
    let horario = format!("{:02}:{:02}", data.hora, data.minuto);
    match numero_do_dia(hoje) - numero_do_dia(data) {
        0 => format!("Hoje, {horario}"),
        1 => format!("Ontem, {horario}"),
        _ => {
            let mes = MESES.get(data.mes.saturating_sub(1) as usize).copied().unwrap_or("");
            format!("{} {mes}, {horario}", data.dia)
        }
    }
}

fn numero_do_dia(data: DataLocal) -> i64 {
    let (ano, mes, dia) = (i64::from(data.ano), i64::from(data.mes), i64::from(data.dia));
    let ano_ajustado = if mes <= 2 { ano - 1 } else { ano };
    let mes_ajustado = (mes + 9) % 12;
    365 * ano_ajustado + ano_ajustado / 4 - ano_ajustado / 100 + ano_ajustado / 400 + (153 * mes_ajustado + 2) / 5 + dia
}

fn comeca_com_carimbo(texto: &str) -> bool {
    const MOLDE: &[u8] = b"0000-00-00 00-00-00";
    let bytes = texto.as_bytes();
    bytes.len() >= MOLDE.len()
        && MOLDE.iter().zip(bytes).all(|(molde, byte)| {
            if *molde == b'0' {
                byte.is_ascii_digit()
            } else {
                byte == molde
            }
        })
}

fn eh_clipe_concluido(caminho: &Path) -> bool {
    let nome = caminho.file_name().and_then(|nome| nome.to_str()).unwrap_or_default();
    nome.ends_with(".mp4") && !nome.ends_with(".parcial.mp4")
}

fn ordenar(clipes: &mut [Clipe], ordem: OrdemClipes) {
    match ordem {
        OrdemClipes::Recentes => clipes.sort_by(|a, b| b.modificado.cmp(&a.modificado)),
        OrdemClipes::Antigos => clipes.sort_by(|a, b| a.modificado.cmp(&b.modificado)),
        OrdemClipes::Maiores => clipes.sort_by(|a, b| b.bytes.cmp(&a.bytes)),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::time::Duration;

    fn clipe(titulo: &str, segundos: u64, bytes: u64) -> Clipe {
        Clipe {
            titulo: titulo.into(),
            jogo: None,
            automatico: false,
            caminho: PathBuf::from(titulo),
            bytes,
            modificado: SystemTime::UNIX_EPOCH + Duration::from_secs(segundos),
            miniatura: None,
            duracao: None,
        }
    }

    fn data(dia: u16, mes: u16, hora: u16) -> DataLocal {
        DataLocal {
            ano: 2026,
            mes,
            dia,
            hora,
            minuto: 7,
        }
    }

    #[test]
    fn identifica_jogo_pelos_metadados_ou_nome_original() {
        assert_eq!(identificar_jogo(Some("VALORANT"), "Ace"), Some("VALORANT".into()));
        assert_eq!(
            identificar_jogo(None, "Fortnite 2026-11-06 12-00-00"),
            Some("Fortnite".into())
        );
        assert_eq!(identificar_jogo(None, "Minha jogada"), None);
        assert_eq!(identificar_jogo(Some("Desconhecido"), "Ace"), None);
    }

    #[test]
    fn filtros_incluem_apenas_jogos_com_clipes_e_recentes_respeitam_o_jogo() {
        let mut antigo = clipe("Primeiro", 1, 10);
        antigo.jogo = Some("VALORANT".into());
        let mut recente = clipe("Segundo", 3, 20);
        recente.jogo = Some("VALORANT".into());
        let mut outro = clipe("Outro jogo", 4, 30);
        outro.jogo = Some("Fortnite".into());
        let clipes = vec![antigo, outro, recente, clipe("Sem jogo", 2, 40)];
        assert_eq!(jogos_com_clipes(&clipes), ["Fortnite", "VALORANT"]);
        let filtrados = ultimos_clipes(&clipes, Some("VALORANT"), 1);
        assert_eq!(filtrados.len(), 1);
        assert_eq!(filtrados[0].titulo, "Segundo");
        assert_eq!(ultimos_clipes(&clipes, None, 12).len(), 4);
        assert_eq!(ultimos_clipes(&clipes, None, 12)[0].titulo, "Outro jogo");
        assert!(jogos_com_clipes(&[]).is_empty());
        assert!(ultimos_clipes(&clipes, Some("PUBG"), 12).is_empty());
    }

    #[test]
    fn titulo_vem_do_que_antecede_o_carimbo() {
        assert_eq!(titulo_do_arquivo("VALORANT 2026-11-06 16-52-31"), "VALORANT");
        assert_eq!(
            titulo_do_arquivo("Counter Strike 2 2026-11-06 16-52-31 2"),
            "Counter Strike 2"
        );
    }

    #[test]
    fn nome_editado_pelo_usuario_e_mantido_inteiro() {
        assert_eq!(titulo_do_arquivo("Zaga insana 1v4"), "Zaga insana 1v4");
    }

    #[test]
    fn ignora_arquivos_incompletos_e_outros_formatos() {
        assert!(eh_clipe_concluido(Path::new("a.mp4")));
        assert!(!eh_clipe_concluido(Path::new("a.parcial.mp4")));
        assert!(!eh_clipe_concluido(Path::new("a.txt")));
    }

    #[test]
    fn busca_ignora_caixa_e_ordem_e_aplicada() {
        let clipes = vec![clipe("Ace", 1, 10), clipe("Clutch", 2, 30), clipe("ACE final", 3, 20)];
        let achados = filtrar_clipes(&clipes, "ace", OrdemClipes::Maiores);
        let titulos: Vec<&str> = achados.iter().map(|clipe| clipe.titulo.as_str()).collect();
        assert_eq!(titulos, ["ACE final", "Ace"]);
        assert_eq!(filtrar_clipes(&clipes, "", OrdemClipes::Antigos)[0].titulo, "Ace");
    }

    #[test]
    fn formata_tempo_e_tamanho() {
        assert_eq!(formatar_duracao(125 * UNIDADES_POR_SEGUNDO), "2:05");
        assert_eq!(formatar_tamanho(106 * 1024 * 1024), "106 MB");
        assert_eq!(formatar_tamanho(1536 * 1024 * 1024), "1,5 GB");
    }

    #[test]
    fn titulo_automatico_preserva_pontuacao_e_renomeacao_manual() {
        let sufixo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let pasta = std::env::temp_dir().join(format!("zaga_nome_evento_{}_{sufixo}", std::process::id()));
        std::fs::create_dir(&pasta).unwrap();
        let arquivo = pasta.join("4K Ancient 2026-11-06 12-00-00.mp4");
        std::fs::write(&arquivo, [0]).unwrap();
        let dados = serde_json::json!({"titulo":"4K: Ancient","jogo":"Counter Strike 2","partida":"p",
            "rodada":"1","inicio_ms":1,"fim_ms":2,"eliminacoes":4,"mapa":"Ancient","vitoria":false});
        std::fs::write(arquivo.with_extension("json"), serde_json::to_vec(&dados).unwrap()).unwrap();
        assert_eq!(listar_clipes(&pasta, &pasta, &pasta)[0].titulo, "4K: Ancient");
        assert!(listar_clipes(&pasta, &pasta, &pasta)[0].automatico);
        let renomeado = pasta.join("Minha jogada.mp4");
        std::fs::rename(&arquivo, &renomeado).unwrap();
        assert_eq!(listar_clipes(&pasta, &pasta, &pasta)[0].titulo, "Minha jogada");
        assert!(!listar_clipes(&pasta, &pasta, &pasta)[0].automatico);
        std::fs::remove_file(renomeado).unwrap();
        assert!(!arquivo.with_extension("json").exists());
        std::fs::remove_dir_all(pasta.join("metadados")).unwrap();
        std::fs::remove_dir(pasta).unwrap();
    }

    #[test]
    fn data_relativa_atravessa_virada_de_mes() {
        assert_eq!(formatar_data(data(6, 11, 21), data(6, 11, 23)), "Hoje, 21:07");
        assert_eq!(formatar_data(data(31, 10, 9), data(1, 11, 23)), "Ontem, 09:07");
        assert_eq!(formatar_data(data(14, 1, 22), data(6, 11, 23)), "14 jan, 22:07");
    }
}
