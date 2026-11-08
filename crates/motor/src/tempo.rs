use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::SystemInformation::GetLocalTime;
use zaga_nucleo::UNIDADES_POR_SEGUNDO;

static REFERENCIA: OnceLock<(i64, i64)> = OnceLock::new();

/// Retorna UTC para integrar eventos de serviços externos e metadados dos clipes.
pub fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// Converte um evento UTC para a base monotônica fixada no início do processo.
pub fn de_unix_ms(instante: i64) -> i64 {
    let (utc, qpc) = *REFERENCIA.get_or_init(|| (unix_ms(), agora()));
    qpc + instante.saturating_sub(utc).saturating_mul(10_000)
}

/// Instante atual em unidades de 100 ns, na mesma base usada pela captura de vídeo e de áudio.
pub fn agora() -> i64 {
    let mut contador = 0;
    let mut frequencia = 0;
    unsafe {
        let _ = QueryPerformanceCounter(&mut contador);
        let _ = QueryPerformanceFrequency(&mut frequencia);
    }
    assert!(frequencia > 0, "contador de alta resolução indisponível");
    (i128::from(contador) * i128::from(UNIDADES_POR_SEGUNDO) / i128::from(frequencia)) as i64
}

pub fn carimbo_para_nome_de_arquivo() -> String {
    let data: SYSTEMTIME = unsafe { GetLocalTime() };
    format!(
        "{:04}-{:02}-{:02} {:02}-{:02}-{:02}",
        data.wYear, data.wMonth, data.wDay, data.wHour, data.wMinute, data.wSecond
    )
}
