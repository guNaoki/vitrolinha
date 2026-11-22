//! Timer para dormir: pausa a música depois de alguns minutos.
//!
//! Uma thread por timer, parada em `recv_timeout` (sem polling). Trocar ou cancelar o timer
//! solta o `Sender` antigo, e a thread antiga acorda sozinha e termina.

use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

/// Opções do menu (minutos).
pub const OPTIONS_MIN: [u64; 5] = [15, 30, 45, 60, 90];

#[derive(Default)]
pub struct SleepState {
    /// Quando a música para + o canal que mantém a thread viva.
    active: Mutex<Option<(Instant, Sender<()>)>>,
}

/// Programa a pausa para daqui a `minutes` (substitui o timer anterior).
pub fn start(app: &AppHandle, minutes: u64) {
    let (tx, rx) = channel::<()>();
    let deadline = Instant::now() + Duration::from_secs(minutes * 60);
    *app.state::<SleepState>().active.lock().unwrap() = Some((deadline, tx));
    let app = app.clone();
    thread::Builder::new()
        .name("sleep-timer".into())
        .spawn(move || {
            if let Err(RecvTimeoutError::Timeout) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                fire(&app, deadline);
            }
        })
        .expect("não foi possível criar a thread do timer");
}

pub fn cancel(app: &AppHandle) {
    *app.state::<SleepState>().active.lock().unwrap() = None;
}

/// Minutos que faltam (arredondado para cima), se há timer.
pub fn remaining_min(app: &AppHandle) -> Option<u64> {
    let state = app.state::<SleepState>();
    let deadline = state.active.lock().unwrap().as_ref().map(|(d, _)| *d)?;
    Some(deadline.saturating_duration_since(Instant::now()).as_secs().div_ceil(60).max(1))
}

fn fire(app: &AppHandle, deadline: Instant) {
    {
        // Só vale se este ainda é o timer atual (não foi trocado nem cancelado no último instante).
        let state = app.state::<SleepState>();
        let mut active = state.active.lock().unwrap();
        if active.as_ref().map(|(d, _)| *d) != Some(deadline) {
            return;
        }
        *active = None;
    }
    crate::media::pause_if_playing(app);
}
