//! Ícone na bandeja: popup (ligar, posição, tempo), painel (ligar, monitor,
//! tamanho), iniciar com o Windows e sair. É a tela de configurações do app:
//! sem janela extra, sem WebView a mais.

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter};
use tauri_plugin_autostart::ManagerExt;

use crate::settings::{self, MonitorRef};
use crate::{hdcover, lyrics, media, shortcuts};
use crate::windows;

const TRAY_ID: &str = "main";
const ID_POPUP: &str = "popup";
const ID_POPUP_PLAY_PAUSE: &str = "popup-play-pause";
const ID_PANEL: &str = "panel";
const ID_FILL: &str = "fill";
const ID_DISCORD: &str = "discord";
const ID_DYNAMIC_COLOR: &str = "dynamic-color";
const ID_LYRICS: &str = "lyrics";
const ID_HD_COVER: &str = "hd-cover";
const ID_HISTORY: &str = "history";
const ID_CLEAR_HISTORY: &str = "clear-history";
const ID_SHORTCUTS: &str = "shortcuts";
const ID_AUTOSTART: &str = "autostart";
const ID_QUIT: &str = "quit";
const PREFIX_MONITOR: &str = "monitor:";
const PREFIX_SOURCE: &str = "source:";
const PREFIX_FX: &str = "fx:";
const PREFIX_POSITION: &str = "popup-pos:";
const PREFIX_DURATION: &str = "popup-dur:";

const POSITIONS: [(&str, &str); 6] = [
    ("top-left", "Em cima, à esquerda"),
    ("top-center", "Em cima, centralizado"),
    ("top-right", "Em cima, à direita"),
    ("bottom-left", "Embaixo, à esquerda"),
    ("bottom-center", "Embaixo, centralizado"),
    ("bottom-right", "Embaixo, à direita"),
];
const DURATIONS_MS: [u64; 4] = [2_000, 4_000, 6_000, 10_000];

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Flyout")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| match event {
            // Clique esquerdo: liga/desliga o painel.
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => {
                let app = tray.app_handle();
                windows::toggle_panel(app);
                refresh_menu(app);
            }
            // O menu é estático: reconstrói ao passar o mouse, para refletir
            // monitores conectados/desconectados e o estado do autostart.
            TrayIconEvent::Enter { .. } => refresh_menu(tray.app_handle()),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn refresh_menu(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn check(app: &AppHandle, id: &str, label: &str, checked: bool) -> tauri::Result<CheckMenuItem<tauri::Wry>> {
    CheckMenuItem::with_id(app, id, label, true, checked, None::<&str>)
}

fn popup_menu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let current_pos = settings::popup_position(app);
    let position = Submenu::with_id(app, "popup-position", "Posição", true)?;
    for (value, label) in POSITIONS {
        position.append(&check(app, &format!("{PREFIX_POSITION}{value}"), label, current_pos == value)?)?;
    }

    let current_ms = settings::popup_duration_ms(app);
    let duration = Submenu::with_id(app, "popup-duration", "Tempo na tela", true)?;
    for ms in DURATIONS_MS {
        let label = format!("{} s", ms / 1000);
        duration.append(&check(app, &format!("{PREFIX_DURATION}{ms}"), &label, current_ms == ms)?)?;
    }

    let menu = Submenu::with_id(app, "popup-menu", "Popup", true)?;
    menu.append(&check(app, ID_POPUP, "Mostrar ao trocar de faixa", settings::popup_enabled(app))?)?;
    menu.append(&check(app, ID_POPUP_PLAY_PAUSE, "Mostrar também ao pausar/tocar", settings::popup_on_play_pause(app))?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&position)?;
    menu.append(&duration)?;
    Ok(menu)
}

fn panel_menu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let current = windows::target_monitor(app).map(|m| windows::monitor_ref(&m));
    let monitors = Submenu::with_id(app, "monitors", "Monitor", true)?;
    for (i, m) in windows::monitors(app).iter().enumerate() {
        let r = windows::monitor_ref(m);
        let label = format!(
            "Monitor {} — {}×{}{}",
            i + 1,
            m.size().width,
            m.size().height,
            if windows::is_primary(app, m) { " (principal)" } else { "" }
        );
        // O id carrega a posição e o nome: sobrevive a mudanças na lista.
        let id = format!("{PREFIX_MONITOR}{}:{}:{}", r.x, r.y, r.name.clone().unwrap_or_default());
        monitors.append(&check(app, &id, &label, current.as_ref() == Some(&r))?)?;
    }

    let menu = Submenu::with_id(app, "panel-menu", "Painel", true)?;
    menu.append(&check(app, ID_PANEL, "Mostrar painel", settings::panel_enabled(app))?)?;
    menu.append(&monitors)?;
    // Desligado = janela livre: arraste pela faixa de cima e redimensione pelas bordas.
    menu.append(&check(app, ID_FILL, "Ocupar o monitor inteiro", settings::panel_fills_monitor(app))?)?;
    Ok(menu)
}

/// "Fonte de mídia": todos os apps, ou só um (ex.: YouTube Music). Evita que vídeos
/// do navegador tomem o lugar quando o player principal pausa.
fn source_menu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let chosen = settings::media_source(app);
    let menu = Submenu::with_id(app, "sources", "Fonte de mídia", true)?;
    menu.append(&check(app, PREFIX_SOURCE, "Todos os apps", chosen.is_none())?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    let mut listed = Vec::new();
    for s in media::sources(app) {
        let id = format!("{PREFIX_SOURCE}{}", s.id);
        menu.append(&check(app, &id, &format!("Só {}", s.label), chosen.as_deref() == Some(s.id.as_str()))?)?;
        listed.push(s.id);
    }
    // A fonte escolhida pode estar fechada agora: continua marcada, para dar para desmarcar.
    if let Some(id) = chosen.filter(|c| !listed.contains(c)) {
        let label = format!("Só {} (fechado)", media::friendly_name(&id));
        menu.append(&check(app, &format!("{PREFIX_SOURCE}{id}"), &label, true)?)?;
    }
    Ok(menu)
}

/// "Estante": guardar o que você ouve (só neste computador) e apagar o histórico.
fn shelf_menu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let menu = Submenu::with_id(app, "shelf", "Estante", true)?;
    menu.append(&check(app, ID_HISTORY, "Guardar o que eu ouço (só neste computador)", settings::history_enabled(app))?)?;
    menu.append(&MenuItem::with_id(app, ID_CLEAR_HISTORY, "Limpar a estante (apaga o histórico)", true, None::<&str>)?)?;
    Ok(menu)
}

/// "Efeitos do disco": intensidade do brilho que gira, dos riscos e do tremor do braço.
fn fx_menu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let current = settings::disk_fx(app);
    let menu = Submenu::with_id(app, "fx", "Efeitos do disco", true)?;
    for (value, label) in [("off", "Desligados"), ("soft", "Suaves"), ("strong", "Fortes")] {
        menu.append(&check(app, &format!("{PREFIX_FX}{value}"), label, current == value)?)?;
    }
    Ok(menu)
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let dynamic = check(app, ID_DYNAMIC_COLOR, "Cor de destaque da capa", settings::dynamic_color(app))?;
    let lyrics = check(app, ID_LYRICS, "Letras sincronizadas (consulta lrclib.net)", settings::lyrics_enabled(app))?;
    let hd_cover = check(app, ID_HD_COVER, "Capa em alta resolução (consulta iTunes)", settings::hd_cover(app))?;
    let discord = check(app, ID_DISCORD, "Call do Discord na tela cheia (precisa de configuração)", settings::discord_enabled(app))?;
    let shortcuts = check(
        app,
        ID_SHORTCUTS,
        &format!("Atalhos globais ({})", shortcuts::SUMMARY),
        settings::shortcuts_enabled(app),
    )?;
    let autostart = check(app, ID_AUTOSTART, "Iniciar com o Windows", app.autolaunch().is_enabled().unwrap_or(false))?;
    let quit = MenuItem::with_id(app, ID_QUIT, "Sair", true, None::<&str>)?;
    Menu::with_items(
        app,
        &[
            &source_menu(app)?,
            &popup_menu(app)?,
            &panel_menu(app)?,
            &fx_menu(app)?,
            &shelf_menu(app)?,
            &dynamic,
            &lyrics,
            &hd_cover,
            &discord,
            &shortcuts,
            &autostart,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        ID_POPUP => settings::set_popup_enabled(app, !settings::popup_enabled(app)),
        ID_POPUP_PLAY_PAUSE => settings::set_popup_on_play_pause(app, !settings::popup_on_play_pause(app)),
        ID_PANEL => windows::set_panel_enabled(app, !settings::panel_enabled(app)),
        ID_FILL => windows::set_fills_monitor(app, !settings::panel_fills_monitor(app)),
        ID_DYNAMIC_COLOR => {
            settings::set_dynamic_color(app, !settings::dynamic_color(app));
            media::refresh(app); // aplica na hora, sem esperar a próxima faixa
        }
        ID_LYRICS => {
            settings::set_lyrics_enabled(app, !settings::lyrics_enabled(app));
            lyrics::reload(app); // limpa ou busca a letra da faixa atual na hora
        }
        ID_HISTORY => settings::set_history_enabled(app, !settings::history_enabled(app)),
        ID_CLEAR_HISTORY => crate::history::clear(app),
        ID_HD_COVER => {
            settings::set_hd_cover(app, !settings::hd_cover(app));
            hdcover::reload(app); // busca (ou deixa de usar) a capa grande da faixa atual
        }
        ID_DISCORD => {
            settings::set_discord_enabled(app, !settings::discord_enabled(app));
            crate::discord::reload(app); // conecta (ou desconecta) na hora
        }
        ID_SHORTCUTS => {
            settings::set_shortcuts_enabled(app, !settings::shortcuts_enabled(app));
            shortcuts::apply(app);
        }
        ID_AUTOSTART => {
            let launch = app.autolaunch();
            let result = if launch.is_enabled().unwrap_or(false) { launch.disable() } else { launch.enable() };
            if let Err(e) = result {
                eprintln!("[autostart] falha: {e}");
            }
        }
        ID_QUIT => app.exit(0),
        _ => {
            if let Some(source) = id.strip_prefix(PREFIX_SOURCE) {
                // "source:" vazio = todos os apps
                media::set_source(app, (!source.is_empty()).then(|| source.to_string()));
            } else if let Some(level) = id.strip_prefix(PREFIX_FX).filter(|l| settings::DISK_FX_LEVELS.contains(l)) {
                settings::set_disk_fx(app, level);
                let _ = app.emit("disk-fx", level); // painel e popup aplicam na hora
            } else if let Some(pos) = id.strip_prefix(PREFIX_POSITION) {
                settings::set_popup_position(app, pos);
            } else if let Some(ms) = id.strip_prefix(PREFIX_DURATION).and_then(|v| v.parse().ok()) {
                settings::set_popup_duration_ms(app, ms);
            } else if let Some(rest) = id.strip_prefix(PREFIX_MONITOR) {
                let mut parts = rest.splitn(3, ':');
                if let (Some(x), Some(y), Some(name)) = (parts.next(), parts.next(), parts.next()) {
                    if let (Ok(x), Ok(y)) = (x.parse(), y.parse()) {
                        let name = (!name.is_empty()).then(|| name.to_string());
                        windows::choose_monitor(app, &MonitorRef { name, x, y });
                    }
                }
            }
        }
    }
    refresh_menu(app); // reflete o novo estado nos checks
}
