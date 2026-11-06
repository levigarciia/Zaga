use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::SystemInformation::GetLocalTime;
use zaga_nucleo::UNIDADES_POR_SEGUNDO;

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
