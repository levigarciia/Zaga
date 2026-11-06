use crate::armazenamento::AnelCompartilhado;
use crate::codificador_audio::CodificadorAudio;
use crate::registro::registrar;
use crate::tempo::agora;
use crate::tipos_midia::{BYTES_POR_QUADRO_AUDIO, CANAIS_AUDIO, DescritorAudio, TAXA_AUDIO};
use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
    IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator, WAVE_FORMAT_PCM, WAVEFORMATEX,
    eCapture, eConsole, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use zaga_nucleo::UNIDADES_POR_SEGUNDO;
use zaga_nucleo::anel::Fluxo;
use zaga_nucleo::mistura::FilaMicrofone;
use zaga_nucleo::relogio_audio::{Correcao, RelogioAudio};

const DURACAO_BUFFER_DISPOSITIVO: i64 = UNIDADES_POR_SEGUNDO / 5;
const ESPERA_POR_PACOTE_MS: u32 = 200;
const ESPERA_COM_MICROFONE_MS: u32 = 50;
const PREFIXO_ERRO_MICROFONE: &str = "Microfone indisponível:";
const ESPERA_ANTES_DE_REABRIR: Duration = Duration::from_secs(2);

type FilaCompartilhada = Arc<Mutex<FilaMicrofone>>;

/// Mantém a trilha de áudio, reabrindo o dispositivo quando ele é removido ou trocado.
///
/// O loopback dita o tempo da trilha mesmo com o som do sistema desligado, e o microfone é misturado a ele.
pub fn executar(
    anel: AnelCompartilhado,
    descritor: Arc<OnceLock<DescritorAudio>>,
    parar: Arc<AtomicBool>,
    incluir_sistema: bool,
    microfone: Option<FilaCompartilhada>,
) {
    if let Err(erro) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        registrar(format!("audio desativado: {erro}"));
        return;
    }
    while !parar.load(Ordering::Relaxed) {
        if let Err(erro) = capturar_ate_falhar(&anel, &descritor, &parar, incluir_sistema, microfone.as_ref()) {
            registrar(format!("captura de audio interrompida: {erro:#}"));
            std::thread::sleep(ESPERA_ANTES_DE_REABRIR);
        }
    }
}

/// Mantém a captura do microfone padrão, informando a falha ao usuário e tentando de novo até conseguir.
pub fn executar_microfone(fila: FilaCompartilhada, parar: Arc<AtomicBool>, ultimo_erro: Arc<Mutex<Option<String>>>) {
    if let Err(erro) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        registrar(format!("microfone desativado: {erro}"));
        return;
    }
    while !parar.load(Ordering::Relaxed) {
        if let Err(erro) = capturar_microfone_ate_falhar(&fila, &parar, &ultimo_erro) {
            let mensagem = format!("{PREFIXO_ERRO_MICROFONE} {erro:#}");
            registrar(&mensagem);
            if let Ok(mut atual) = ultimo_erro.lock() {
                *atual = Some(mensagem);
            }
            std::thread::sleep(ESPERA_ANTES_DE_REABRIR);
        }
    }
}

fn capturar_microfone_ate_falhar(
    fila: &Mutex<FilaMicrofone>,
    parar: &AtomicBool,
    ultimo_erro: &Mutex<Option<String>>,
) -> Result<()> {
    let dispositivo = DispositivoAudio::abrir(OrigemAudio::Microfone)?;
    if let Ok(mut atual) = ultimo_erro.lock()
        && atual
            .as_deref()
            .is_some_and(|erro| erro.starts_with(PREFIXO_ERRO_MICROFONE))
    {
        *atual = None;
    }
    while !parar.load(Ordering::Relaxed) {
        if dispositivo.aguardar(ESPERA_POR_PACOTE_MS) {
            dispositivo.encher_fila(fila)?;
        }
    }
    Ok(())
}

fn capturar_ate_falhar(
    anel: &AnelCompartilhado,
    descritor: &OnceLock<DescritorAudio>,
    parar: &AtomicBool,
    incluir_sistema: bool,
    microfone: Option<&FilaCompartilhada>,
) -> Result<()> {
    let codificador = CodificadorAudio::new()?;
    let _ = descritor.set(codificador.descritor().clone());
    let dispositivo = DispositivoAudio::abrir(OrigemAudio::Sistema)?;
    let espera = if microfone.is_some() {
        ESPERA_COM_MICROFONE_MS
    } else {
        ESPERA_POR_PACOTE_MS
    };
    let mut trilha = TrilhaAudio {
        codificador,
        anel,
        relogio: RelogioAudio::new(TAXA_AUDIO),
        silencio: Vec::new(),
        mistura: Vec::new(),
        incluir_sistema,
        microfone: microfone.cloned(),
    };

    while !parar.load(Ordering::Relaxed) {
        if dispositivo.aguardar(espera) {
            dispositivo.consumir_pacotes(&mut trilha)?;
        } else {
            trilha.preencher_silencio_ate(agora())?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OrigemAudio {
    Sistema,
    Microfone,
}

struct DispositivoAudio {
    cliente: IAudioClient,
    captura: IAudioCaptureClient,
    evento: HANDLE,
}

impl DispositivoAudio {
    fn abrir(origem: OrigemAudio) -> Result<Self> {
        let enumerador: IMMDeviceEnumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }?;
        let dispositivo = match origem {
            OrigemAudio::Sistema => unsafe { enumerador.GetDefaultAudioEndpoint(eRender, eConsole) }
                .context("nenhum dispositivo de saida de audio")?,
            OrigemAudio::Microfone => unsafe { enumerador.GetDefaultAudioEndpoint(eCapture, eConsole) }
                .context("nenhum microfone encontrado")?,
        };
        let cliente: IAudioClient = unsafe { dispositivo.Activate(CLSCTX_ALL, None) }?;

        let bloco = (CANAIS_AUDIO * 2) as u16;
        let formato = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_PCM as u16,
            nChannels: CANAIS_AUDIO as u16,
            nSamplesPerSec: TAXA_AUDIO,
            nAvgBytesPerSec: TAXA_AUDIO * u32::from(bloco),
            nBlockAlign: bloco,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        let opcoes_comuns = AUDCLNT_STREAMFLAGS_EVENTCALLBACK
            | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
            | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
        let opcoes = match origem {
            OrigemAudio::Sistema => opcoes_comuns | AUDCLNT_STREAMFLAGS_LOOPBACK,
            OrigemAudio::Microfone => opcoes_comuns,
        };
        unsafe {
            cliente.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                opcoes,
                DURACAO_BUFFER_DISPOSITIVO,
                0,
                &formato,
                None,
            )
        }
        .context("falha ao abrir o dispositivo de audio")?;

        let evento = unsafe { CreateEventW(None, false, false, None) }?;
        unsafe { cliente.SetEventHandle(evento) }?;
        let captura: IAudioCaptureClient = unsafe { cliente.GetService() }?;
        unsafe { cliente.Start() }?;
        Ok(Self {
            cliente,
            captura,
            evento,
        })
    }

    fn aguardar(&self, espera_ms: u32) -> bool {
        unsafe { WaitForSingleObject(self.evento, espera_ms) == WAIT_OBJECT_0 }
    }

    fn encher_fila(&self, fila: &Mutex<FilaMicrofone>) -> Result<()> {
        while unsafe { self.captura.GetNextPacketSize() }? > 0 {
            let mut dados = std::ptr::null_mut();
            let mut quadros = 0;
            let mut sinalizadores = 0;
            unsafe {
                self.captura
                    .GetBuffer(&mut dados, &mut quadros, &mut sinalizadores, None, None)
            }?;
            if let Ok(mut fila) = fila.lock() {
                if sinalizadores & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                    fila.empurrar_silencio((quadros * CANAIS_AUDIO) as usize);
                } else {
                    let bytes = (quadros * BYTES_POR_QUADRO_AUDIO) as usize;
                    fila.empurrar(unsafe { std::slice::from_raw_parts(dados, bytes) });
                }
            }
            unsafe { self.captura.ReleaseBuffer(quadros) }?;
        }
        Ok(())
    }

    fn consumir_pacotes(&self, trilha: &mut TrilhaAudio<'_>) -> Result<()> {
        while unsafe { self.captura.GetNextPacketSize() }? > 0 {
            let mut dados = std::ptr::null_mut();
            let mut quadros = 0;
            let mut sinalizadores = 0;
            let mut instante = 0u64;
            unsafe {
                self.captura
                    .GetBuffer(&mut dados, &mut quadros, &mut sinalizadores, None, Some(&mut instante))
            }?;
            let silencioso = sinalizadores & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
            let bytes = (quadros * BYTES_POR_QUADRO_AUDIO) as usize;
            let pcm = (!silencioso).then(|| unsafe { std::slice::from_raw_parts(dados, bytes) });
            let resultado = trilha.processar_bloco(pcm, quadros, instante as i64);
            unsafe { self.captura.ReleaseBuffer(quadros) }?;
            resultado?;
        }
        Ok(())
    }
}

impl Drop for DispositivoAudio {
    fn drop(&mut self) {
        unsafe {
            let _ = self.cliente.Stop();
            let _ = CloseHandle(self.evento);
        }
    }
}

struct TrilhaAudio<'a> {
    codificador: CodificadorAudio,
    anel: &'a AnelCompartilhado,
    relogio: RelogioAudio,
    silencio: Vec<u8>,
    mistura: Vec<u8>,
    incluir_sistema: bool,
    microfone: Option<FilaCompartilhada>,
}

impl TrilhaAudio<'_> {
    fn processar_bloco(&mut self, pcm: Option<&[u8]>, quadros: u32, instante: i64) -> Result<()> {
        let pcm = pcm.filter(|_| self.incluir_sistema);
        let mut pular = 0;
        match self.relogio.avaliar(instante) {
            Correcao::Nenhuma => {}
            Correcao::InserirSilencio { quadros } => self.emitir_silencio(quadros, instante)?,
            Correcao::DescartarQuadros { quadros: excedentes } => pular = excedentes.min(quadros),
        }
        let restantes = quadros - pular;
        if restantes == 0 {
            return Ok(());
        }
        match pcm {
            Some(bytes) => self.emitir(&bytes[(pular * BYTES_POR_QUADRO_AUDIO) as usize..], restantes, instante),
            None => self.emitir_silencio(restantes, instante),
        }
    }

    fn preencher_silencio_ate(&mut self, instante: i64) -> Result<()> {
        if let Correcao::InserirSilencio { quadros } = self.relogio.avaliar(instante) {
            self.emitir_silencio(quadros, instante)?;
        }
        Ok(())
    }

    fn emitir_silencio(&mut self, quadros: u32, instante: i64) -> Result<()> {
        let mut buffer = std::mem::take(&mut self.silencio);
        buffer.clear();
        buffer.resize((quadros * BYTES_POR_QUADRO_AUDIO) as usize, 0);
        let resultado = self.emitir(&buffer, quadros, instante);
        self.silencio = buffer;
        resultado
    }

    fn emitir(&mut self, pcm: &[u8], quadros: u32, instante: i64) -> Result<()> {
        let pts = self.relogio.emitir(quadros, instante);
        let duracao = i64::from(quadros) * UNIDADES_POR_SEGUNDO / i64::from(TAXA_AUDIO);
        let mut mistura = std::mem::take(&mut self.mistura);
        mistura.clear();
        mistura.extend_from_slice(pcm);
        if let Some(Ok(mut fila)) = self.microfone.as_ref().map(|fila| fila.lock()) {
            fila.misturar(&mut mistura);
        }
        let resultado = self.codificador.codificar(&mistura, pts, duracao, |dados, pts_pacote| {
            let mut anel = self.anel.lock().map_err(|_| anyhow::anyhow!("anel envenenado"))?;
            anel.inserir(Fluxo::Audio, pts_pacote, false, dados)?;
            Ok(())
        });
        self.mistura = mistura;
        resultado
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    #[ignore = "Requer acesso ao microfone padrão do Windows"]
    fn microfone_real_entrega_amostras() -> Result<()> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
        let dispositivo = DispositivoAudio::abrir(OrigemAudio::Microfone)?;
        let fila = Mutex::new(FilaMicrofone::default());
        let limite = std::time::Instant::now() + Duration::from_secs(3);
        let mut recebeu = false;
        while std::time::Instant::now() < limite {
            if dispositivo.aguardar(ESPERA_POR_PACOTE_MS) && unsafe { dispositivo.captura.GetNextPacketSize() }? > 0 {
                dispositivo.encher_fila(&fila)?;
                recebeu = true;
                break;
            }
        }
        anyhow::ensure!(recebeu, "O microfone não entregou amostras em três segundos");
        Ok(())
    }
}
