// Sem janela de console no release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod accent;
mod artwork;
mod discord;
mod fullscreen;
mod hdcover;
mod history;
mod logfile;
mod lyrics;
mod media;
mod popup;
mod settings;
mod shortcuts;
mod tray;
mod i18n;
mod overlay;
mod presence;
mod share;
mod sleep;
mod volume;
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

/// Barra de progresso do painel: pula para essa posição (ms desde o início da faixa).
#[tauri::command]
fn seek(app: tauri::AppHandle, position_ms: i64) -> Result<(), String> {
    media::seek(&app, position_ms)
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

/// Estante: os últimos discos ouvidos (mais recente primeiro).
#[tauri::command]
fn get_history(app: tauri::AppHandle) -> Vec<history::Record> {
    history::list(&app)
}

/// Volume do sistema (controle de volume do painel). `None` se não há dispositivo de saída.
#[tauri::command]
fn get_volume() -> Option<volume::Volume> {
    volume::get()
}

#[tauri::command]
fn set_volume(level: f32) -> Option<volume::Volume> {
    volume::set(level)
}

#[tauri::command]
fn toggle_mute() -> Option<volume::Volume> {
    volume::toggle_mute()
}

/// Call de voz do Discord (lista na tela cheia); mudanças chegam pelo evento `voice`.
#[tauri::command]
fn get_voice(app: tauri::AppHandle) -> discord::Voice {
    discord::current(&app)
}

/// Nível dos efeitos do disco ("off" | "soft" | "strong"); mudanças chegam pelo evento `disk-fx`.
#[tauri::command]
fn get_disk_fx(app: tauri::AppHandle) -> String {
    settings::disk_fx(&app)
}

/// Opções (as mesmas do menu da bandeja) para a tela de opções do painel.
#[tauri::command]
fn get_menu(app: tauri::AppHandle) -> Vec<tray::Entry> {
    tray::model(&app)
}

/// Clique numa opção da tela de opções (mesmo efeito do clique no menu da bandeja).
#[tauri::command]
fn menu_click(app: tauri::AppHandle, id: String) {
    tray::click(&app, &id);
}

/// Idioma da interface ("pt" | "en"); mudanças chegam pelo evento `lang` (as janelas recarregam).
#[tauri::command]
fn get_lang(app: tauri::AppHandle) -> &'static str {
    i18n::lang(&app).code()
}

/// Rotação do disco e chiado de vinil; mudanças chegam pelo evento `vinyl`.
#[tauri::command]
fn get_vinyl(app: tauri::AppHandle) -> settings::Vinyl {
    settings::vinyl(&app)
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
        // mostra o painel; `vitrolinha.exe --popup` dispara o popup (útil para testar
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
        .manage(sleep::SleepState::default())
        .manage(presence::PresenceState::default())
        .manage(overlay::OverlayState::default())
        .manage(lyrics::LyricsState::default())
        .manage(hdcover::HdState::default())
        .manage(history::HistoryState::default())
        .manage(discord::DiscordState::default())
        .invoke_handler(tauri::generate_handler![
            get_state,
            get_voice,
            get_lyrics,
            toggle_play_pause,
            next,
            previous,
            seek,
            minimize_panel,
            toggle_panel_size,
            get_disk_fx,
            get_vinyl,
            get_lang,
            get_menu,
            menu_click,
            share::read_cover,
            share::save_share_image,
            share::copy_share_image,
            get_history,
            get_volume,
            set_volume,
            toggle_mute,
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
            logfile::init(handle); // primeiro: o que vier depois já cai no arquivo de log
            tray::init(handle)?;
            windows::apply_panel_visibility(handle);
            popup::init(handle);
            shortcuts::apply(handle);
            media::start(handle.clone());
            discord::reload(handle);
            overlay::reload(handle); // overlay para OBS, se ligado
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erro ao iniciar a Vitrolinha");
}
