//! Janela do painel: escolha do monitor, posição/tamanho, mostrar/esconder.
//! (O popup entra no Marco 4.)

use std::sync::atomic::{AtomicU64, Ordering};

use tauri::{AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::settings::{self, MonitorRef};

pub const PANEL: &str = "panel";
/// Tamanho de referência do painel (lógico) quando não ocupa o monitor.
const FIXED_W: f64 = 1280.0;
const FIXED_H: f64 = 720.0;

pub fn panel(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(PANEL)
}

pub fn monitors(app: &AppHandle) -> Vec<Monitor> {
    app.available_monitors().unwrap_or_default()
}

pub fn monitor_ref(m: &Monitor) -> MonitorRef {
    MonitorRef { name: m.name().cloned(), x: m.position().x, y: m.position().y }
}

pub fn is_primary(app: &AppHandle, m: &Monitor) -> bool {
    app.primary_monitor()
        .ok()
        .flatten()
        .is_some_and(|p| p.position() == m.position() && p.size() == m.size())
}

/// Monitor do painel: o salvo (nome+posição, depois nome, depois posição);
/// sem escolha salva, o primeiro que não é o principal (o "2º monitor");
/// com um monitor só, ele mesmo.
pub fn target_monitor(app: &AppHandle) -> Option<Monitor> {
    let list = monitors(app);
    if let Some(saved) = settings::panel_monitor(app) {
        let same_name = |m: &Monitor| m.name().cloned() == saved.name;
        let same_pos = |m: &Monitor| m.position().x == saved.x && m.position().y == saved.y;
        let found = list
            .iter()
            .find(|m| same_name(m) && same_pos(m))
            .or_else(|| list.iter().find(|m| same_name(m)))
            .or_else(|| list.iter().find(|m| same_pos(m)));
        if let Some(m) = found {
            return Some(m.clone());
        }
    }
    list.iter().find(|m| !is_primary(app, m)).or_else(|| list.first()).cloned()
}

/// Tamanho mínimo da janela livre (lógico).
const MIN_W: f64 = 360.0;
const MIN_H: f64 = 260.0;

/// O painel está no modo de janela livre (arrastar e redimensionar)?
pub fn is_free(app: &AppHandle) -> bool {
    !settings::panel_fills_monitor(app)
}

/// Posiciona/dimensiona o painel no monitor escolhido.
/// - Modo "monitor inteiro": cobre o monitor, sem arrastar nem redimensionar.
/// - Modo livre: usa o retângulo lembrado (se ainda está no monitor escolhido);
///   senão 1280×720 centralizado. O usuário arrasta/redimensiona à vontade.
pub fn place_panel(app: &AppHandle) {
    let (Some(win), Some(mon)) = (panel(app), target_monitor(app)) else {
        return;
    };
    let (pos, size) = (*mon.position(), *mon.size());
    let free = is_free(app);

    let (target_pos, target_size) = if !free {
        (pos, size)
    } else {
        let saved = settings::panel_rect(app).filter(|r| rect_center_in(r, &mon));
        match saved {
            Some(r) => (PhysicalPosition::new(r.x, r.y), PhysicalSize::new(r.w, r.h)),
            None => {
                let k = mon.scale_factor();
                let w = ((FIXED_W * k).round() as u32).min(size.width);
                let h = ((FIXED_H * k).round() as u32).min(size.height);
                (
                    PhysicalPosition::new(
                        pos.x + ((size.width - w) / 2) as i32,
                        pos.y + ((size.height - h) / 2) as i32,
                    ),
                    PhysicalSize::new(w, h),
                )
            }
        }
    };

    let _ = win.set_resizable(free);
    let _ = win.set_min_size(free.then(|| tauri::LogicalSize::new(MIN_W, MIN_H)));
    // DPI misto: ao cruzar para um monitor de outra escala o sistema pode
    // reescalar a janela; por isso posição → tamanho → posição, tudo físico.
    let _ = win.set_position(target_pos);
    let _ = win.set_size(target_size);
    let _ = win.set_position(target_pos);
    let _ = app.emit("panel-free", free); // o front liga/desliga a barra de arrastar
}

fn rect_center_in(r: &settings::RectRef, mon: &Monitor) -> bool {
    let (cx, cy) = (r.x as i64 + (r.w / 2) as i64, r.y as i64 + (r.h / 2) as i64);
    let (p, s) = (mon.position(), mon.size());
    cx >= p.x as i64 && cx < p.x as i64 + s.width as i64 && cy >= p.y as i64 && cy < p.y as i64 + s.height as i64
}

/// Moveu/redimensionou (modo livre): lembra o retângulo, depois de 500 ms sem
/// novos eventos (arrastar gera dezenas de eventos por segundo).
pub fn remember_rect(app: &AppHandle) {
    static EVENTS: AtomicU64 = AtomicU64::new(0);
    if !is_free(app) {
        return;
    }
    let this_event = EVENTS.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if EVENTS.load(Ordering::SeqCst) != this_event {
            return; // veio outro evento depois
        }
        let Some(win) = panel(&app) else { return };
        // Minimizada/escondida a janela reporta posição absurda: não guarda.
        if win.is_minimized().unwrap_or(true) || !win.is_visible().unwrap_or(false) {
            return;
        }
        if let (Ok(p), Ok(s)) = (win.outer_position(), win.outer_size()) {
            settings::set_panel_rect(&app, &settings::RectRef { x: p.x, y: p.y, w: s.width, h: s.height });
        }
    });
}

/// Clique no ícone da bandeja: se o painel está minimizado, restaura; senão,
/// liga/desliga.
pub fn toggle_panel(app: &AppHandle) {
    let minimized = panel(app).is_some_and(|w| w.is_minimized().unwrap_or(false));
    if minimized && settings::panel_enabled(app) {
        apply_panel_visibility(app); // restaura
    } else {
        set_panel_enabled(app, !settings::panel_enabled(app));
    }
}

/// Liga/desliga o painel (persistente). Esconder pausa o front antes.
pub fn set_panel_enabled(app: &AppHandle, on: bool) {
    settings::set_panel_enabled(app, on);
    apply_panel_visibility(app);
}

/// Mostra ou esconde conforme a configuração salva.
pub fn apply_panel_visibility(app: &AppHandle) {
    let Some(win) = panel(app) else { return };
    if settings::panel_enabled(app) {
        place_panel(app);
        let _ = win.unminimize(); // se estava minimizado, volta
        let _ = win.show();
        let _ = app.emit("panel-visible", true);
    } else {
        // Avisa o front primeiro: animações e timers param antes de esconder.
        let _ = app.emit("panel-visible", false);
        let _ = win.hide();
    }
}

pub fn choose_monitor(app: &AppHandle, m: &MonitorRef) {
    settings::set_panel_monitor(app, m); // (place_panel descarta o retângulo se ficou em outro monitor)
    if settings::panel_enabled(app) {
        place_panel(app);
    }
}

pub fn set_fills_monitor(app: &AppHandle, on: bool) {
    settings::set_panel_fills_monitor(app, on);
    if settings::panel_enabled(app) {
        place_panel(app);
    }
}
