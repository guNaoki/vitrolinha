//! Popup que aparece quando a faixa troca.
//!
//! Uma thread dona do ciclo de vida: fica bloqueada (0% de CPU) enquanto o
//! popup está oculto; só enquanto ele está na tela acorda a cada `TICK` para
//! tratar o auto-hide e o click-through. Nada roda com o popup escondido.

use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::{fullscreen, settings};

const LABEL: &str = "popup";
/// Janela (lógica) = card 520×120 + margem de 30px para a sombra.
const WIN_W: f64 = 676.0;
const WIN_H: f64 = 180.0;
const INSET: f64 = 30.0;
/// Distância do card até a borda da área de trabalho.
const EDGE: f64 = 24.0;

/// Passo de verificação enquanto visível.
const TICK: Duration = Duration::from_millis(100);
/// Duração da animação de saída no front (deixe um pouco acima do CSS).
const EXIT: Duration = Duration::from_millis(340);
/// Com o mouse em cima, o popup não some antes disso.
const HOVER_GRACE: Duration = Duration::from_millis(1_500);

pub struct PopupHandle(Mutex<Sender<()>>);

pub fn init(app: &AppHandle) {
    let (tx, rx) = channel();
    app.manage(PopupHandle(Mutex::new(tx)));
    let app = app.clone();
    thread::Builder::new()
        .name("popup".into())
        .spawn(move || run(app, rx))
        .expect("não foi possível criar a thread do popup");
}

/// Pede para mostrar (ou renovar) o popup.
pub fn trigger(app: &AppHandle) {
    if let Some(h) = app.try_state::<PopupHandle>() {
        let _ = h.0.lock().unwrap().send(());
    }
}

enum Phase {
    Hidden,
    Visible { deadline: Instant },
    Leaving { until: Instant },
}

fn run(app: AppHandle, rx: Receiver<()>) {
    let mut phase = Phase::Hidden;
    let mut click_through = true; // estado atual de ignore_cursor_events

    loop {
        let triggered = match phase {
            Phase::Hidden => match rx.recv() {
                Ok(()) => true,
                Err(_) => return,
            },
            _ => match rx.recv_timeout(TICK) {
                Ok(()) => true,
                Err(RecvTimeoutError::Timeout) => false,
                Err(RecvTimeoutError::Disconnected) => return,
            },
        };
        let now = Instant::now();

        if triggered {
            while rx.try_recv().is_ok() {} // agrupa pedidos em rajada
            if settings::popup_enabled(&app) {
                if let Some(why) = fullscreen::suppress_reason() {
                    println!("[popup] suprimido: {why}");
                } else {
                    let duration = Duration::from_millis(settings::popup_duration_ms(&app));
                    if matches!(phase, Phase::Visible { .. }) {
                        phase = Phase::Visible { deadline: now + duration }; // renova
                    } else if show(&app) {
                        click_through = true;
                        phase = Phase::Visible { deadline: now + duration };
                    }
                }
            }
        }

        match phase {
            Phase::Visible { deadline } => {
                let hover = hovering(&app);
                // Click-through, exceto com o mouse em cima (para o botão funcionar).
                if hover == click_through {
                    set_click_through(&app, !hover);
                    click_through = !hover;
                }
                let deadline = if hover { deadline.max(now + HOVER_GRACE) } else { deadline };
                phase = if now >= deadline {
                    let _ = app.emit_to(LABEL, "popup-leave", ());
                    Phase::Leaving { until: now + EXIT }
                } else {
                    Phase::Visible { deadline }
                };
            }
            Phase::Leaving { until } if now >= until => {
                hide(&app);
                phase = Phase::Hidden;
            }
            _ => {}
        }
    }
}

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

fn show(app: &AppHandle) -> bool {
    let Some(win) = window(app) else { return false };
    place(app, &win);
    let _ = win.set_ignore_cursor_events(true);
    let _ = win.show(); // a janela é `focusable: false`: não rouba o foco
    let _ = app.emit_to(LABEL, "popup-visible", true);
    true
}

fn hide(app: &AppHandle) {
    // Avisa antes: o front pausa animações e timers.
    let _ = app.emit_to(LABEL, "popup-visible", false);
    if let Some(win) = window(app) {
        let _ = win.hide();
    }
}

fn set_click_through(app: &AppHandle, on: bool) {
    if let Some(win) = window(app) {
        let _ = win.set_ignore_cursor_events(on);
    }
}

/// O cursor está sobre o card (não só sobre a margem transparente)?
fn hovering(app: &AppHandle) -> bool {
    let Some(win) = window(app) else { return false };
    let (Ok(cur), Ok(pos), Ok(size)) = (app.cursor_position(), win.outer_position(), win.outer_size())
    else {
        return false;
    };
    let inset = INSET * win.scale_factor().unwrap_or(1.0);
    let (x0, y0) = (pos.x as f64 + inset, pos.y as f64 + inset);
    let (x1, y1) = (pos.x as f64 + size.width as f64 - inset, pos.y as f64 + size.height as f64 - inset);
    cur.x >= x0 && cur.x <= x1 && cur.y >= y0 && cur.y <= y1
}

/// Monitor "ativo": o que tem o cursor; senão o principal.
fn active_monitor(app: &AppHandle) -> Option<Monitor> {
    let monitors = app.available_monitors().unwrap_or_default();
    if let Ok(cur) = app.cursor_position() {
        let hit = monitors.iter().find(|m| {
            let (p, s) = (m.position(), m.size());
            cur.x >= p.x as f64
                && cur.x < p.x as f64 + s.width as f64
                && cur.y >= p.y as f64
                && cur.y < p.y as f64 + s.height as f64
        });
        if let Some(m) = hit {
            return Some(m.clone());
        }
    }
    app.primary_monitor().ok().flatten().or_else(|| monitors.into_iter().next())
}

fn place(app: &AppHandle, win: &WebviewWindow) {
    let Some(mon) = active_monitor(app) else { return };
    let k = mon.scale_factor();
    let wa = mon.work_area();
    let (wa_x, wa_y) = (wa.position.x, wa.position.y);
    let (wa_w, wa_h) = (wa.size.width as i32, wa.size.height as i32);

    let w = (WIN_W * k).round() as i32;
    let h = (WIN_H * k).round() as i32;
    // O card fica EDGE px da borda: a janela sobra INSET − EDGE para fora.
    let out = ((INSET - EDGE) * k).round() as i32;

    let position = settings::popup_position(app);
    let (v, hz) = position.split_once('-').unwrap_or(("bottom", "center"));
    let x = match hz {
        "left" => wa_x - out,
        "right" => wa_x + wa_w - w + out,
        _ => wa_x + (wa_w - w) / 2,
    };
    let y = match v {
        "top" => wa_y - out,
        _ => wa_y + wa_h - h + out,
    };

    let _ = win.set_size(PhysicalSize::new(w as u32, h as u32));
    let _ = win.set_position(PhysicalPosition::new(x, y));
    // DPI misto: repete a posição depois do tamanho (o sistema pode reescalar
    // ao cruzar para um monitor de outra escala).
    let _ = win.set_position(PhysicalPosition::new(x, y));
}
