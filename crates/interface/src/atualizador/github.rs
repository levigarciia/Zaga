use anyhow::{Context, Result, ensure};
use reqwest::blocking::Client;
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

const API: &str = "https://api.github.com/repos/levigarciia/Zaga/releases/latest";
const BASE_DOWNLOAD: &str = "https://github.com/levigarciia/Zaga/releases/download";
const LIMITE_EXECUTAVEL: u64 = 128 * 1024 * 1024;

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
    size: u64,
}

#[derive(Clone, Debug)]
pub(super) struct Atualizacao {
    pub versao: String,
    pub url: String,
    pub sha256: String,
    pub tamanho: u64,
}

pub(super) fn consultar(versao_atual: &str) -> Result<Option<Atualizacao>> {
    let cliente = cliente(Duration::from_secs(20))?;
    let resposta = cliente.get(API).send()?.error_for_status()?;
    let release: Release = serde_json::from_reader(resposta.take(2 * 1024 * 1024))?;
    selecionar(release, versao_atual)
}

fn selecionar(release: Release, versao_atual: &str) -> Result<Option<Atualizacao>> {
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let versao = release.tag_name.strip_prefix('v').context("Tag de versão inválida")?;
    let nova = Version::parse(versao).context("Versão publicada inválida")?;
    let atual = Version::parse(versao_atual)?;
    if !nova.pre.is_empty() || !nova.build.is_empty() || nova <= atual {
        return Ok(None);
    }
    let nome = format!("ZAGA-{versao}-x64.exe");
    let asset = release
        .assets
        .into_iter()
        .find(|asset| asset.name == nome)
        .context("Executável de atualização ausente na release")?;
    let url = format!("{BASE_DOWNLOAD}/{}/{nome}", release.tag_name);
    ensure!(asset.browser_download_url == url, "Origem de atualização inválida");
    ensure!(
        (1..=LIMITE_EXECUTAVEL).contains(&asset.size),
        "Tamanho de atualização inválido"
    );
    let sha256 = asset
        .digest
        .as_deref()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .context("SHA256 da atualização ausente")?;
    validar_hash(sha256)?;
    Ok(Some(Atualizacao {
        versao: versao.to_owned(),
        url,
        sha256: sha256.to_ascii_lowercase(),
        tamanho: asset.size,
    }))
}

fn cliente(tempo: Duration) -> Result<Client> {
    Ok(Client::builder()
        .user_agent(concat!("ZAGA/", env!("CARGO_PKG_VERSION")))
        .https_only(true)
        .connect_timeout(Duration::from_secs(10))
        .timeout(tempo)
        .redirect(reqwest::redirect::Policy::custom(|tentativa| {
            let permitida = tentativa.url().scheme() == "https"
                && matches!(
                    tentativa.url().host_str(),
                    Some("github.com" | "release-assets.githubusercontent.com" | "objects.githubusercontent.com")
                );
            if permitida && tentativa.previous().len() < 5 {
                tentativa.follow()
            } else {
                tentativa.error("Redirecionamento de atualização recusado")
            }
        }))
        .build()?)
}

pub(super) fn baixar(atualizacao: &Atualizacao, destino: &Path) -> Result<()> {
    let resposta = cliente(Duration::from_secs(180))?
        .get(&atualizacao.url)
        .send()?
        .error_for_status()?;
    let mut arquivo = std::fs::File::create_new(destino)?;
    let resultado = copiar_validado(resposta, &mut arquivo, atualizacao.tamanho, &atualizacao.sha256);
    if resultado.is_ok() {
        arquivo.sync_all()?;
    }
    drop(arquivo);
    if resultado.is_err() {
        let _ = std::fs::remove_file(destino);
    }
    resultado
}

pub(super) fn verificar_arquivo(caminho: &Path, tamanho: u64, hash: &str) -> Result<()> {
    copiar_validado(std::fs::File::open(caminho)?, std::io::sink(), tamanho, hash)
}

fn validar_hash(hash: &str) -> Result<()> {
    ensure!(
        hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "SHA256 inválido"
    );
    Ok(())
}

fn copiar_validado(mut origem: impl Read, mut destino: impl Write, tamanho: u64, hash: &str) -> Result<()> {
    validar_hash(hash)?;
    ensure!((1..=LIMITE_EXECUTAVEL).contains(&tamanho), "Tamanho inválido");
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut bloco = [0; 64 * 1024];
    loop {
        let quantidade = origem.read(&mut bloco)?;
        if quantidade == 0 {
            break;
        }
        total += quantidade as u64;
        ensure!(total <= tamanho, "Atualização excede o tamanho publicado");
        digest.update(&bloco[..quantidade]);
        destino.write_all(&bloco[..quantidade])?;
    }
    ensure!(total == tamanho, "Download incompleto");
    ensure!(
        format!("{:x}", digest.finalize()).eq_ignore_ascii_case(hash),
        "SHA256 da atualização não confere"
    );
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    #[ignore = "Consulta e baixa a release pública do GitHub"]
    fn consulta_e_valida_release_publicada() {
        let atualizacao = consultar("0.0.0").unwrap().expect("Release pública com atualizador");
        let pasta = tempfile::tempdir().unwrap();
        let arquivo = pasta.path().join("ZAGA.exe");
        baixar(&atualizacao, &arquivo).unwrap();
        verificar_arquivo(&arquivo, atualizacao.tamanho, &atualizacao.sha256).unwrap();
        assert_eq!(&std::fs::read(arquivo).unwrap()[..2], b"MZ");
    }

    fn release(versao: &str) -> Release {
        let nome = format!("ZAGA-{versao}-x64.exe");
        Release {
            tag_name: format!("v{versao}"),
            draft: false,
            prerelease: false,
            assets: vec![Asset {
                browser_download_url: format!("{BASE_DOWNLOAD}/v{versao}/{nome}"),
                name: nome,
                digest: Some(format!("sha256:{}", "a".repeat(64))),
                size: 1024,
            }],
        }
    }

    #[test]
    fn compara_versoes_sem_downgrade_e_ignora_pre_releases() {
        assert!(selecionar(release("0.10.0"), "0.9.0").unwrap().is_some());
        assert!(selecionar(release("0.2.0"), "0.2.0").unwrap().is_none());
        assert!(selecionar(release("0.1.0"), "0.2.0").unwrap().is_none());
        assert!(selecionar(release("0.3.0-beta.1"), "0.2.0").unwrap().is_none());
        let mut rascunho = release("0.3.0");
        rascunho.draft = true;
        assert!(selecionar(rascunho, "0.2.0").unwrap().is_none());
    }

    #[test]
    fn rejeita_release_sem_integridade_ou_com_origem_adulterada() {
        let mut sem_hash = release("0.3.0");
        sem_hash.assets[0].digest = None;
        assert!(selecionar(sem_hash, "0.2.0").is_err());
        let mut outra_origem = release("0.3.0");
        outra_origem.assets[0].browser_download_url = "https://outro.example/ZAGA.exe".into();
        assert!(selecionar(outra_origem, "0.2.0").is_err());
        let mut tamanho = release("0.3.0");
        tamanho.assets[0].size = LIMITE_EXECUTAVEL + 1;
        assert!(selecionar(tamanho, "0.2.0").is_err());
    }

    #[test]
    fn verifica_download_truncado_corrompido_e_maior_que_o_declarado() {
        let dados = b"ZAGA";
        let hash = format!("{:x}", Sha256::digest(dados));
        let mut saida = Vec::new();
        copiar_validado(&dados[..], &mut saida, 4, &hash).unwrap();
        assert_eq!(saida, dados);
        assert!(copiar_validado(&dados[..3], std::io::sink(), 4, &hash).is_err());
        assert!(copiar_validado(&dados[..], std::io::sink(), 3, &hash).is_err());
        assert!(copiar_validado(&b"ZAGO"[..], std::io::sink(), 4, &hash).is_err());
    }
}
