//! Volume do sistema (o mesmo da barra de tarefas), pelo Core Audio.
//!
//! O Windows não expõe volume por app para o player (o GSMTC só controla a
//! reprodução), então o controle do painel mexe no volume geral do dispositivo de
//! saída padrão. Nada roda em segundo plano: o front consulta quando o painel está
//! visível e depois de cada ajuste.

use serde::Serialize;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    /// 0.0 – 1.0
    pub level: f32,
    pub muted: bool,
}

/// Dispositivo de saída padrão (pode mudar entre chamadas: fones, HDMI...).
fn endpoint() -> windows::core::Result<IAudioEndpointVolume> {
    // Chamado de threads do Tauri: garante COM (o erro "já inicializado de outro modo" não importa).
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
    }
}

fn read(ep: &IAudioEndpointVolume) -> windows::core::Result<Volume> {
    unsafe { Ok(Volume { level: ep.GetMasterVolumeLevelScalar()?, muted: ep.GetMute()?.as_bool() }) }
}

pub fn get() -> Option<Volume> {
    read(&endpoint().ok()?).ok()
}

/// Ajusta o volume (0–1). Subir o volume tira do mudo, como o Windows faz.
pub fn set(level: f32) -> Option<Volume> {
    let level = level.clamp(0.0, 1.0);
    let ep = endpoint().ok()?;
    unsafe {
        ep.SetMasterVolumeLevelScalar(level, std::ptr::null()).ok()?;
        if level > 0.0 && ep.GetMute().ok()?.as_bool() {
            ep.SetMute(false, std::ptr::null()).ok()?;
        }
    }
    read(&ep).ok()
}

pub fn toggle_mute() -> Option<Volume> {
    let ep = endpoint().ok()?;
    unsafe {
        let muted = ep.GetMute().ok()?.as_bool();
        ep.SetMute(!muted, std::ptr::null()).ok()?;
    }
    read(&ep).ok()
}
