use crate::inicializacao::definir_inicio_com_windows;
use crate::origem::origem_em_primeiro_plano;
use crate::registro::registrar;
use crate::sessao::Sessao;
use crate::uso_cpu::MedidorCpu;
use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MESSAGEBOX_STYLE};
use zaga_nucleo::atalho::Atalho;
use zaga_nucleo::config::Config;
use zaga_nucleo::protocolo::EstadoMotor;

/// Ponto único de comando do motor, compartilhado entre atalhos, bandeja e interface.
pub struct Controlador {
    pasta_dados: PathBuf,
    pasta_clipes: PathBuf,
    config: Mutex<Config>,
    sessao: Mutex<Option<Sessao>>,
    ultimo_erro: Arc<Mutex<Option<String>>>,
    medidor_cpu: Mutex<MedidorCpu>,
    atalhos_indisponiveis: Mutex<Vec<usize>>,
}

impl Controlador {
    pub fn new(pasta_dados: PathBuf, pasta_clipes: PathBuf) -> Self {
        let config = carregar_config(&caminho_config(&pasta_dados));
        Self {
            pasta_dados,
            pasta_clipes,
            config: Mutex::new(config),
            sessao: Mutex::new(None),
            ultimo_erro: Arc::new(Mutex::new(None)),
            medidor_cpu: Mutex::new(MedidorCpu::new()),
            atalhos_indisponiveis: Mutex::new(Vec::new()),
        }
    }

    pub fn iniciar_captura(&self) {
        let config = self.config.lock().map(|config| config.clone()).unwrap_or_default();
        if !config.captura_ativa {
            self.definir_erro(None);
            return;
        }
        let resultado = Sessao::iniciar(&config, &self.pasta_dados, self.ultimo_erro.clone());
        match resultado {
            Ok(sessao) => {
                if let Ok(mut atual) = self.sessao.lock() {
                    *atual = Some(sessao);
                }
                self.definir_erro(None);
            }
            Err(erro) => self.definir_erro(Some(format!("{erro:#}"))),
        }
    }

    pub fn encerrar_captura(&self) {
        if let Ok(mut sessao) = self.sessao.lock() {
            sessao.take();
        }
    }

    pub fn salvar_clipe(&self, segundos: u32) -> Result<()> {
        let sessao = self.sessao.lock().map_err(|_| anyhow!("sessao envenenada"))?;
        let sessao = sessao
            .as_ref()
            .filter(|sessao| sessao.trabalhando())
            .context("captura parada")?;
        let ultimo_erro = self.ultimo_erro.clone();
        let origem = origem_em_primeiro_plano();
        sessao.salvar_em_segundo_plano(segundos, origem, self.pasta_clipes.clone(), move |resultado| {
            concluir_salvamento(resultado, &ultimo_erro);
        })
    }

    pub fn aplicar_config(&self, nova: Config) -> Result<()> {
        validar_atalhos(&nova.atalhos)?;
        let anterior = self.config.lock().map(|config| config.clone()).unwrap_or_default();
        if anterior.iniciar_com_windows != nova.iniciar_com_windows {
            definir_inicio_com_windows(nova.iniciar_com_windows)?;
        }
        escrever_config(&caminho_config(&self.pasta_dados), &nova)?;
        let reiniciar = anterior.requer_reinicio(&nova);
        let atalhos_mudaram = anterior.atalhos != nova.atalhos;
        if let Ok(mut config) = self.config.lock() {
            *config = nova;
        }
        if atalhos_mudaram {
            crate::janela::reaplicar_atalhos();
        }
        if reiniciar {
            self.encerrar_captura();
            self.iniciar_captura();
        }
        Ok(())
    }

    pub fn estado(&self) -> EstadoMotor {
        let sessao = self.sessao.lock().ok();
        let resumo = sessao
            .as_ref()
            .and_then(|sessao| sessao.as_ref())
            .filter(|sessao| sessao.trabalhando());
        EstadoMotor {
            jogo_ativo: crate::origem::jogo_em_primeiro_plano(),
            sessao: resumo.map(Sessao::resumo),
            config: self.config.lock().map(|config| config.clone()).unwrap_or_default(),
            pasta_clipes: self.pasta_clipes.to_string_lossy().into_owned(),
            pasta_dados: self.pasta_dados.to_string_lossy().into_owned(),
            uso_cpu_decimos: self.medidor_cpu.lock().map_or(0, |mut medidor| medidor.medir()),
            atalhos_indisponiveis: self
                .atalhos_indisponiveis
                .lock()
                .map(|lista| lista.clone())
                .unwrap_or_default(),
            ultimo_erro: self.ultimo_erro.lock().ok().and_then(|erro| erro.clone()),
        }
    }

    pub fn atalhos(&self) -> [Atalho; 3] {
        self.config
            .lock()
            .map(|config| config.atalhos.clone())
            .unwrap_or_else(|_| Atalho::padroes())
    }

    pub fn definir_atalhos_indisponiveis(&self, indices: Vec<usize>) {
        if let Ok(mut atual) = self.atalhos_indisponiveis.lock() {
            *atual = indices;
        }
    }

    pub fn pasta_clipes(&self) -> &Path {
        &self.pasta_clipes
    }

    fn definir_erro(&self, erro: Option<String>) {
        if let Some(mensagem) = &erro {
            registrar(mensagem);
        }
        if let Ok(mut atual) = self.ultimo_erro.lock() {
            *atual = erro;
        }
    }
}

fn concluir_salvamento(resultado: Result<PathBuf>, ultimo_erro: &Mutex<Option<String>>) {
    let (estilo, erro): (MESSAGEBOX_STYLE, Option<String>) = match resultado {
        Ok(caminho) => {
            registrar(format!("clipe salvo: {}", caminho.display()));
            (MB_OK, None)
        }
        Err(erro) => {
            let mensagem = format!("falha ao salvar clipe: {erro:#}");
            registrar(&mensagem);
            (MB_ICONERROR, Some(mensagem))
        }
    };
    let _ = unsafe { MessageBeep(estilo) };
    if let Ok(mut atual) = ultimo_erro.lock() {
        *atual = erro;
    }
}

fn caminho_config(pasta_dados: &Path) -> PathBuf {
    pasta_dados.join("config.json")
}

fn validar_atalhos(atalhos: &[Atalho; 3]) -> Result<()> {
    for (indice, atalho) in atalhos.iter().enumerate() {
        anyhow::ensure!(atalho.valido(), "Combinação de atalho inválida: {}", atalho.descricao());
        let duplicado = atalhos[..indice].iter().any(|anterior| {
            anterior.codigo_virtual() == atalho.codigo_virtual()
                && anterior.ctrl == atalho.ctrl
                && anterior.alt == atalho.alt
                && anterior.shift == atalho.shift
        });
        anyhow::ensure!(!duplicado, "Combinação de atalho repetida: {}", atalho.descricao());
    }
    Ok(())
}

fn carregar_config(caminho: &Path) -> Config {
    let Ok(texto) = std::fs::read_to_string(caminho) else {
        return Config::default();
    };
    serde_json::from_str(&texto).unwrap_or_else(|erro| {
        registrar(format!("config invalida, usando padrao: {erro}"));
        Config::default()
    })
}

fn escrever_config(caminho: &Path, config: &Config) -> Result<()> {
    let temporario = caminho.with_extension("json.tmp");
    std::fs::write(&temporario, serde_json::to_vec_pretty(config)?).context("falha ao gravar a config")?;
    std::fs::rename(&temporario, caminho).context("falha ao substituir a config")?;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn recusa_tecla_sem_modificador_e_combinacao_repetida() {
        let mut atalhos = Atalho::padroes();
        assert!(validar_atalhos(&atalhos).is_ok());
        atalhos[0].alt = false;
        atalhos[0].tecla = "k".into();
        assert!(validar_atalhos(&atalhos).is_err());
        atalhos = Atalho::padroes();
        atalhos[1] = atalhos[0].clone();
        assert!(validar_atalhos(&atalhos).is_err());
        atalhos[1].tecla = "f09".into();
        assert!(validar_atalhos(&atalhos).is_err());
    }
}
