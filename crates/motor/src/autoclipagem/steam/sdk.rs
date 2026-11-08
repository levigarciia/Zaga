use anyhow::{Result, ensure};
use libloading::{Library, Symbol};
use serde_json::{Value, json};
use std::ffi::{CStr, c_char, c_void};
use std::path::Path;

pub(super) struct Sdk {
    biblioteca: Library,
    estatisticas: *mut c_void,
}

impl Sdk {
    pub(super) fn abrir(caminho: &Path) -> Result<Self> {
        unsafe {
            let biblioteca = Library::new(caminho)?;
            let em_execucao: Symbol<unsafe extern "C" fn() -> bool> = biblioteca.get(b"SteamAPI_IsSteamRunning\0")?;
            ensure!(em_execucao(), "A Steam precisa estar aberta para acompanhar conquistas");
            let iniciar: Symbol<unsafe extern "C" fn(*mut c_char) -> i32> = biblioteca.get(b"SteamAPI_InitFlat\0")?;
            let mut erro = [0i8; 1024];
            ensure!(
                iniciar(erro.as_mut_ptr()) == 0,
                "Steam indisponível ou jogo sem licença na conta ativa"
            );
            let mut sdk = Self {
                biblioteca,
                estatisticas: std::ptr::null_mut(),
            };
            let obter: Symbol<unsafe extern "C" fn() -> *mut c_void> =
                sdk.biblioteca.get(b"SteamAPI_SteamUserStats_v013\0")?;
            sdk.estatisticas = obter();
            ensure!(!sdk.estatisticas.is_null(), "Estatísticas Steam indisponíveis");
            let solicitar: Symbol<unsafe extern "C" fn(*mut c_void) -> u64> = sdk
                .biblioteca
                .get(b"SteamAPI_ISteamUserStats_RequestGlobalAchievementPercentages\0")?;
            solicitar(sdk.estatisticas);
            Ok(sdk)
        }
    }

    pub(super) fn consultar(&self) -> Result<Vec<Value>> {
        unsafe {
            let callbacks: Symbol<unsafe extern "C" fn()> = self.biblioteca.get(b"SteamAPI_RunCallbacks\0")?;
            callbacks();
            let contar: Symbol<unsafe extern "C" fn(*mut c_void) -> u32> =
                self.biblioteca.get(b"SteamAPI_ISteamUserStats_GetNumAchievements\0")?;
            let identificar: Symbol<unsafe extern "C" fn(*mut c_void, u32) -> *const c_char> =
                self.biblioteca.get(b"SteamAPI_ISteamUserStats_GetAchievementName\0")?;
            let estado: Symbol<unsafe extern "C" fn(*mut c_void, *const c_char, *mut bool, *mut u32) -> bool> = self
                .biblioteca
                .get(b"SteamAPI_ISteamUserStats_GetAchievementAndUnlockTime\0")?;
            let percentual: Symbol<unsafe extern "C" fn(*mut c_void, *const c_char, *mut f32) -> bool> = self
                .biblioteca
                .get(b"SteamAPI_ISteamUserStats_GetAchievementAchievedPercent\0")?;
            let atributo: Symbol<unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> *const c_char> =
                self.biblioteca
                    .get(b"SteamAPI_ISteamUserStats_GetAchievementDisplayAttribute\0")?;
            let total = contar(self.estatisticas);
            ensure!(total <= 20_000, "Quantidade de conquistas Steam inválida");
            let mut conquistas = Vec::new();
            for indice in 0..total {
                let id = identificar(self.estatisticas, indice);
                ensure!(!id.is_null(), "Conquista Steam sem identificador");
                let mut desbloqueada = false;
                let mut instante = 0;
                if !estado(self.estatisticas, id, &mut desbloqueada, &mut instante) {
                    continue;
                }
                let mut raridade = -1.0;
                let disponivel = percentual(self.estatisticas, id, &mut raridade);
                let nome = atributo(self.estatisticas, id, c"name".as_ptr());
                conquistas.push(json!({
                    "id": CStr::from_ptr(id).to_string_lossy(),
                    "nome": if nome.is_null() { String::new() } else { CStr::from_ptr(nome).to_string_lossy().into_owned() },
                    "desbloqueada": desbloqueada,
                    "instante": instante,
                    "percentual": if disponivel && raridade.is_finite() && (0.0..=100.0).contains(&raridade) {
                        Some(raridade)
                    } else { None },
                }));
            }
            Ok(conquistas)
        }
    }
}

impl Drop for Sdk {
    fn drop(&mut self) {
        unsafe {
            if let Ok(encerrar) = self.biblioteca.get::<unsafe extern "C" fn()>(b"SteamAPI_Shutdown\0") {
                encerrar();
            }
        }
    }
}
