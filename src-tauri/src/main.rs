// Sem janela de console no release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod accent;
mod artwork;
mod fullscreen;
mod hdcover;
mod lyrics;
mod media;
mod popup;
mod settings;
mod shortcuts;
mod tray;
mod windows;

use media::{Control, NowPlaying, SharedState};
use tauri::{Manager, State, WindowEvent};

#[tauri::command]
fn get_state(app: tauri::AppHandle, state: State<SharedState>) -> Option<NowPlaying> {
    let now = state.now.lock().unwrap().clone();
    now.map(|np| media::for_ui(&app, &np))
}

#[tauri::command]
fn toggle_play_pause(state: State<SharedState>) -> Result<(), String> {
    media::control(&state, Control::TogglePlayPause)
}

#[tauri::command]
fn next(state: State<SharedState>) -> Result<(), String> {
    media::control(&state, Control::Next)
}

#[tauri::command]
fn previous(state: State<SharedState>) -> Result<(), String> {
    media::control(&state, Control::Previous)
}

/// O painel está em modo de janela livre? (o front só mostra a barra de arrastar nele)
#[tauri::command]
fn panel_is_free(app: tauri::AppHandle) -> bool {
    windows::is_free(&app)
}

/// Letra sincronizada da faixa atual, se já chegou.
#[tauri::command]
fn get_lyrics(app: tauri::AppHandle) -> Option<lyrics::Lyrics> {
    lyrics::current(&app)
}

/// Botão "minimizar" do painel: minimiza de verdade (o painel tem botão na barra
/// de tarefas; clicar nele, ou no ícone da bandeja, restaura).
#[tauri::command]
fn minimize_panel(app: tauri::AppHandle) {
    if let Some(win) = windows::panel(&app) {
        let _ = win.minimize();
    }
}

/// Botão "diminuir/ampliar": alterna entre ocupar o monitor inteiro e a janela livre.
#[tauri::command]
fn toggle_panel_size(app: tauri::AppHandle) {
    let free = windows::is_free(&app);
    windows::set_fills_monitor(&app, free); // livre → ocupa o monitor; inteiro → livre
}

fn main() {
    tauri::Builder::default()
        // Instância única: precisa ser o primeiro plugin. Abrir o app de novo
        // mostra o painel; `flyout.exe --popup` dispara o popup (útil para testar
        // posição/duração e para atalhos do sistema).
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if argv.iter().any(|a| a == "--popup") {
                popup::trigger(app);
            } else {
                windows::set_panel_enabled(app, true);
            }
        }))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(SharedState::default())
        .manage(lyrics::LyricsState::default())
        .manage(hdcover::HdState::default())
        .invoke_handler(tauri::generate_handler![
            get_state,
            get_lyrics,
            toggle_play_pause,
            next,
            previous,
            minimize_panel,
            toggle_panel_size,
            panel_is_free
        ])
        .on_window_event(|window, event| {
            if window.label() != windows::PANEL {
                return;
            }
            match event {
                // Fechar (Alt+F4) só esconde o painel; quem encerra é "Sair" na bandeja.
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    windows::set_panel_enabled(window.app_handle(), false);
                }
                // Janela livre: lembra onde o usuário deixou e de que tamanho.
                WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                    windows::remember_rect(window.app_handle());
                }
                _ => {}
            }
        })
        .setup(|app| {
            let handle = app.handle();
            tray::init(handle)?;
            windows::apply_panel_visibility(handle);
            popup::init(handle);
            shortcuts::apply(handle);
            media::start(handle.clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Flyout");
}
