//! Atalhos globais (funcionam com qualquer app em foco).
//!
//! Padrões: Ctrl+Alt+Espaço (tocar/pausar), Ctrl+Alt+→ (próxima), Ctrl+Alt+←
//! (anterior), Ctrl+Alt+↑ (mostrar o popup agora). Cada um pode ser trocado no
//! settings.json (`shortcutPlayPause`, `shortcutNext`, `shortcutPrevious`,
//! `shortcutPopup`); se outro app já usa a combinação, o registro falha e o erro
//! vai para o log.

use std::str::FromStr;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::media::{self, Control, SharedState};
use crate::{popup, settings};

#[derive(Clone, Copy)]
enum Action {
    PlayPause,
    Next,
    Previous,
    ShowPopup,
}

/// (chave no settings.json, combinação padrão, ação)
const BINDINGS: [(&str, &str, Action); 4] = [
    ("shortcutPlayPause", "Ctrl+Alt+Space", Action::PlayPause),
    ("shortcutNext", "Ctrl+Alt+Right", Action::Next),
    ("shortcutPrevious", "Ctrl+Alt+Left", Action::Previous),
    ("shortcutPopup", "Ctrl+Alt+Up", Action::ShowPopup),
];

/// Texto curto para o menu da bandeja.
pub const SUMMARY: &str = "Ctrl+Alt+Espaço, →, ←, ↑";

/// (Re)registra conforme a configuração: chamado na inicialização e ao ligar/desligar.
pub fn apply(app: &AppHandle) {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    if !settings::shortcuts_enabled(app) {
        return;
    }
    for (key, default, action) in BINDINGS {
        let combo = settings::shortcut(app, key, default);
        let shortcut = match Shortcut::from_str(&combo) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[atalhos] combinação inválida em {key}: {combo:?} ({e})");
                continue;
            }
        };
        let result = gs.on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                run(app, action);
            }
        });
        if let Err(e) = result {
            eprintln!("[atalhos] não consegui registrar {combo} ({key}): {e}");
        }
    }
}

fn run(app: &AppHandle, action: Action) {
    match action {
        Action::PlayPause => send(app, Control::TogglePlayPause),
        Action::Next => send(app, Control::Next),
        Action::Previous => send(app, Control::Previous),
        Action::ShowPopup => popup::trigger(app),
    }
}

fn send(app: &AppHandle, what: Control) {
    if let Err(e) = media::control(&app.state::<SharedState>(), what) {
        eprintln!("[atalhos] {what:?}: {e}");
    }
}
