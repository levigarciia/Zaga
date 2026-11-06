use std::time::Instant;
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

const UNIDADES_POR_SEGUNDO: f64 = 10_000_000.0;

/// Mede o uso de CPU do próprio motor entre duas consultas, em décimos de ponto percentual do total da máquina.
pub struct MedidorCpu {
    nucleos: f64,
    ultimo_instante: Instant,
    ultimo_tempo_cpu: u64,
    ultimo_valor: u32,
}

impl MedidorCpu {
    pub fn new() -> Self {
        Self {
            nucleos: std::thread::available_parallelism().map_or(1.0, |nucleos| nucleos.get() as f64),
            ultimo_instante: Instant::now(),
            ultimo_tempo_cpu: tempo_cpu_do_processo(),
            ultimo_valor: 0,
        }
    }

    pub fn medir(&mut self) -> u32 {
        let decorrido = self.ultimo_instante.elapsed().as_secs_f64();
        if decorrido < 0.5 {
            return self.ultimo_valor;
        }
        let atual = tempo_cpu_do_processo();
        let consumido = atual.saturating_sub(self.ultimo_tempo_cpu) as f64 / UNIDADES_POR_SEGUNDO;
        self.ultimo_instante = Instant::now();
        self.ultimo_tempo_cpu = atual;
        self.ultimo_valor = (consumido / decorrido / self.nucleos * 1000.0).round() as u32;
        self.ultimo_valor
    }
}

fn tempo_cpu_do_processo() -> u64 {
    let mut criacao = FILETIME::default();
    let mut saida = FILETIME::default();
    let mut nucleo = FILETIME::default();
    let mut usuario = FILETIME::default();
    let resultado =
        unsafe { GetProcessTimes(GetCurrentProcess(), &mut criacao, &mut saida, &mut nucleo, &mut usuario) };
    if resultado.is_err() {
        return 0;
    }
    para_u64(nucleo) + para_u64(usuario)
}

fn para_u64(tempo: FILETIME) -> u64 {
    (u64::from(tempo.dwHighDateTime) << 32) | u64::from(tempo.dwLowDateTime)
}
