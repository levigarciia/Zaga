use super::github;
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_INVALID_PARAMETER, GetLastError, WAIT_OBJECT_0,
};
use windows::Win32::Storage::FileSystem::{REPLACE_FILE_FLAGS, ReplaceFileW};
use windows::Win32::System::Threading::{CreateMutexW, OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};
use windows::core::HSTRING;

const SEM_JANELA: u32 = 0x08000000;

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Plano {
    pub alvo: PathBuf,
    pub arquivo: PathBuf,
    pub tamanho: u64,
    pub sha256: String,
    pub pid: u32,
    pub reiniciar: bool,
}

pub(super) fn agendar(plano: &Plano) -> Result<()> {
    github::verificar_arquivo(&plano.arquivo, plano.tamanho, &plano.sha256)?;
    let pasta = plano.arquivo.parent().context("Pasta da atualização ausente")?;
    let auxiliar = pasta.join("aplicador.exe");
    let manifesto = pasta.join("plano.json");
    std::fs::write(&manifesto, serde_json::to_vec(plano)?)?;
    std::fs::copy(std::env::current_exe()?, &auxiliar)?;
    Command::new(auxiliar)
        .arg("--aplicar-atualizacao")
        .arg(manifesto)
        .creation_flags(SEM_JANELA)
        .spawn()
        .context("Não foi possível iniciar o aplicador")?;
    Ok(())
}

pub(super) fn executar(manifesto: &Path) -> Result<()> {
    let auxiliar = std::env::current_exe()?.canonicalize()?;
    let manifesto = manifesto.canonicalize()?;
    ensure!(
        manifesto.parent() == auxiliar.parent(),
        "Plano fora da pasta do aplicador"
    );
    ensure!(
        std::fs::metadata(&manifesto)?.len() <= 16 * 1024,
        "Plano de atualização muito grande"
    );
    let plano: Plano = serde_json::from_slice(&std::fs::read(manifesto)?)?;
    let origem = plano.arquivo.canonicalize()?;
    ensure!(
        origem.parent() == auxiliar.parent(),
        "Arquivo fora da pasta do aplicador"
    );
    ensure!(plano.alvo.is_absolute(), "Destino de atualização relativo");
    ensure!(
        plano
            .alvo
            .file_name()
            .is_some_and(|nome| nome.eq_ignore_ascii_case("ZAGA.exe")),
        "Destino não é o executável do ZAGA"
    );
    ensure!(
        plano.pid != std::process::id(),
        "Aplicador não pode aguardar a si mesmo"
    );
    github::verificar_arquivo(&origem, plano.tamanho, &plano.sha256)?;
    aguardar_saida(plano.pid)?;
    let nome = HSTRING::from(zaga_motor::nome_instancia("Local\\ZagaAplicativoInstancia"));
    let exclusividade = unsafe { CreateMutexW(None, false, &nome) }?;
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        let _ = unsafe { CloseHandle(exclusividade) };
        bail!("Outra instância do ZAGA foi aberta; atualização adiada");
    }
    let resultado = instalar_com_retentativas(&plano);
    let _ = unsafe { CloseHandle(exclusividade) };
    if plano.reiniciar && plano.alvo.is_file() {
        Command::new(&plano.alvo)
            .current_dir(plano.alvo.parent().context("Destino sem pasta")?)
            .creation_flags(SEM_JANELA)
            .spawn()
            .context("Atualizado, mas não foi possível reabrir o ZAGA")?;
    }
    resultado
}

fn aguardar_saida(pid: u32) -> Result<()> {
    let processo = match unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) } {
        Ok(processo) => processo,
        Err(erro) if erro.code() == windows::core::HRESULT::from_win32(ERROR_INVALID_PARAMETER.0) => return Ok(()),
        Err(erro) => return Err(erro.into()),
    };
    let resultado = unsafe { WaitForSingleObject(processo, 120_000) };
    let _ = unsafe { CloseHandle(processo) };
    ensure!(
        resultado == WAIT_OBJECT_0,
        "O ZAGA ainda está aberto; atualização adiada"
    );
    Ok(())
}

fn instalar(plano: &Plano) -> Result<()> {
    let pasta = plano.alvo.parent().context("Destino sem pasta")?;
    let nova = pasta.join(format!(".ZAGA_{}.nova", std::process::id()));
    let backup = pasta.join(format!(".ZAGA_{}.anterior", std::process::id()));
    ensure!(
        !nova.exists() && !backup.exists(),
        "Já existe uma substituição pendente nesta pasta"
    );
    let mut destino = std::fs::File::create_new(&nova).context("Pasta do ZAGA sem permissão de escrita")?;
    let resultado = (|| {
        std::io::copy(&mut std::fs::File::open(&plano.arquivo)?, &mut destino)?;
        destino.sync_all()?;
        drop(destino);
        github::verificar_arquivo(&nova, plano.tamanho, &plano.sha256)?;
        substituir(&plano.alvo, &nova, &backup)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&nova);
    resultado
}

fn instalar_com_retentativas(plano: &Plano) -> Result<()> {
    let limite = Instant::now() + Duration::from_secs(15);
    loop {
        let resultado = instalar(plano);
        let Err(erro) = &resultado else { return resultado };
        let bloqueado = erro.downcast_ref::<windows::core::Error>().is_some_and(|erro| {
            [5, 32, 33]
                .into_iter()
                .any(|codigo| erro.code() == windows::core::HRESULT::from_win32(codigo))
        }) || erro
            .downcast_ref::<std::io::Error>()
            .is_some_and(|erro| matches!(erro.raw_os_error(), Some(5 | 32 | 33)));
        if !bloqueado || !plano.alvo.is_file() || Instant::now() >= limite {
            return resultado;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn substituir(alvo: &Path, nova: &Path, backup: &Path) -> Result<()> {
    let resultado = unsafe {
        ReplaceFileW(
            &HSTRING::from(alvo),
            &HSTRING::from(nova),
            &HSTRING::from(backup),
            REPLACE_FILE_FLAGS(0),
            None,
            None,
        )
    };
    if let Err(erro) = resultado {
        if !alvo.exists() && backup.exists() {
            std::fs::rename(backup, alvo).with_context(|| {
                format!(
                    "Falha ao restaurar o executável. Cópia preservada em {}",
                    backup.display()
                )
            })?;
        }
        return Err(anyhow::Error::new(erro).context("Não foi possível substituir o executável"));
    }
    let _ = std::fs::remove_file(backup);
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::os::windows::fs::OpenOptionsExt;

    #[test]
    fn substitui_no_mesmo_disco_sem_deixar_backup() {
        let pasta = tempfile::tempdir().unwrap();
        let alvo = pasta.path().join("ZAGA.exe");
        let nova = pasta.path().join("nova.exe");
        let backup = pasta.path().join("anterior.exe");
        std::fs::write(&alvo, b"anterior").unwrap();
        std::fs::write(&nova, b"nova").unwrap();
        substituir(&alvo, &nova, &backup).unwrap();
        assert_eq!(std::fs::read(&alvo).unwrap(), b"nova");
        assert!(!backup.exists());
    }

    #[test]
    fn preserva_original_quando_o_windows_bloqueia_o_arquivo() {
        let pasta = tempfile::tempdir().unwrap();
        let alvo = pasta.path().join("ZAGA.exe");
        let nova = pasta.path().join("nova.exe");
        let backup = pasta.path().join("anterior.exe");
        std::fs::write(&alvo, b"anterior").unwrap();
        std::fs::write(&nova, b"nova").unwrap();
        let _bloqueio = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&alvo)
            .unwrap();
        assert!(substituir(&alvo, &nova, &backup).is_err());
        drop(_bloqueio);
        assert_eq!(std::fs::read(&alvo).unwrap(), b"anterior");
        let bloqueio = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&alvo)
            .unwrap();
        let liberacao = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            drop(bloqueio);
        });
        let plano = Plano {
            alvo: alvo.clone(),
            arquivo: nova,
            tamanho: 4,
            sha256: format!("{:x}", Sha256::digest(b"nova")),
            pid: 0,
            reiniciar: false,
        };
        instalar_com_retentativas(&plano).unwrap();
        liberacao.join().unwrap();
        assert_eq!(std::fs::read(alvo).unwrap(), b"nova");
    }

    #[test]
    fn rejeita_pacote_corrompido_antes_de_substituir_o_original() {
        let pasta = tempfile::tempdir().unwrap();
        let alvo = pasta.path().join("ZAGA.exe");
        let arquivo = pasta.path().join("download.exe");
        std::fs::write(&alvo, b"anterior").unwrap();
        std::fs::write(&arquivo, b"adulterado").unwrap();
        let plano = Plano {
            alvo: alvo.clone(),
            arquivo,
            tamanho: 4,
            sha256: format!("{:x}", Sha256::digest(b"nova")),
            pid: 0,
            reiniciar: false,
        };
        assert!(instalar(&plano).is_err());
        assert_eq!(std::fs::read(alvo).unwrap(), b"anterior");
        assert_eq!(std::fs::read_dir(pasta.path()).unwrap().count(), 2);
    }
}
