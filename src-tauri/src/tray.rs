//! Ícone na bandeja: popup (ligar, posição, tempo), painel (ligar, monitor,
//! tamanho), iniciar com o Windows e sair. É a tela de configurações do app:
//! sem janela extra, sem WebView a mais.

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter};
use tauri_plugin_autostart::ManagerExt;

use crate::settings::{self, MonitorRef};
use crate::i18n::{self, fmt, tr};
use crate::{hdcover, lyrics, media, shortcuts, sleep};
use crate::windows;

const TRAY_ID: &str = "main";
const ID_POPUP: &str = "popup";
const ID_POPUP_PLAY_PAUSE: &str = "popup-play-pause";
const ID_PANEL: &str = "panel";
const ID_FILL: &str = "fill";
const ID_DISCORD: &str = "discord";
const ID_PRESENCE: &str = "discord-presence";
const ID_OVERLAY: &str = "overlay";
const PREFIX_LANG: &str = "lang:";
const ID_DYNAMIC_COLOR: &str = "dynamic-color";
const ID_LYRICS: &str = "lyrics";
const ID_HD_COVER: &str = "hd-cover";
const ID_HISTORY: &str = "history";
const ID_CLEAR_HISTORY: &str = "clear-history";
const ID_SHORTCUTS: &str = "shortcuts";
const ID_AUTOSTART: &str = "autostart";
const ID_LOGS: &str = "logs";
const ID_QUIT: &str = "quit";
const PREFIX_MONITOR: &str = "monitor:";
const PREFIX_SOURCE: &str = "source:";
const PREFIX_FX: &str = "fx:";
const PREFIX_SLEEP: &str = "sleep:";
const ID_SLEEP_CANCEL: &str = "sleep-cancel";
const PREFIX_RPM: &str = "rpm:";
const PREFIX_CRACKLE: &str = "crackle:";
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
/// Apps que aparecem sempre em "Fonte de mídia": (trecho do identificador, nome).
const KNOWN_SOURCES: [(&str, &str); 2] = [("spotify", "Spotify"), ("music.youtube.com", "YouTube Music")];
const DURATIONS_MS: [u64; 4] = [2_000, 4_000, 6_000, 10_000];

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Vitrolinha")
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

/// Uma linha do menu, em formato que o painel consegue desenhar (a tela de opções do painel).
#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Entry {
    /// Opção ligada/desligada ou uma escolha entre várias (marcada = valor atual).
    Check { id: String, label: String, checked: bool, enabled: bool },
    /// Ação simples.
    Item { id: String, label: String, enabled: bool },
    /// Grupo de opções.
    Sub { id: String, label: String, children: Vec<Entry> },
    Separator,
}

/// O menu da bandeja como dados: a tela de opções do painel mostra exatamente o que o menu
/// tem, sem uma segunda lista para manter em dia.
pub fn model(app: &AppHandle) -> Vec<Entry> {
    match build_menu(app).and_then(|m| m.items()) {
        Ok(items) => items.iter().map(entry).collect(),
        Err(e) => {
            eprintln!("[tray] não foi possível montar o menu: {e}");
            Vec::new()
        }
    }
}

fn entry(kind: &tauri::menu::MenuItemKind<tauri::Wry>) -> Entry {
    use tauri::menu::MenuItemKind as K;
    match kind {
        K::Check(c) => Entry::Check {
            id: c.id().0.clone(),
            label: c.text().unwrap_or_default(),
            checked: c.is_checked().unwrap_or(false),
            enabled: c.is_enabled().unwrap_or(true),
        },
        K::MenuItem(i) => Entry::Item {
            id: i.id().0.clone(),
            label: i.text().unwrap_or_default(),
            enabled: i.is_enabled().unwrap_or(true),
        },
        K::Submenu(s) => Entry::Sub {
            id: s.id().0.clone(),
            label: s.text().unwrap_or_default(),
            children: s.items().map(|v| v.iter().map(entry).collect()).unwrap_or_default(),
        },
        _ => Entry::Separator,
    }
}

/// Clique numa linha da tela de opções: o mesmo caminho do clique no menu da bandeja.
pub fn click(app: &AppHandle, id: &str) {
    on_menu(app, id);
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
    let position = Submenu::with_id(app, "popup-position", tr(app, "Posição"), true)?;
    for (value, label) in POSITIONS {
        position.append(&check(app, &format!("{PREFIX_POSITION}{value}"), &tr(app, label), current_pos == value)?)?;
    }

    let current_ms = settings::popup_duration_ms(app);
    let duration = Submenu::with_id(app, "popup-duration", tr(app, "Tempo na tela"), true)?;
    for ms in DURATIONS_MS {
        let label = format!("{} s", ms / 1000);
        duration.append(&check(app, &format!("{PREFIX_DURATION}{ms}"), &label, current_ms == ms)?)?;
    }

    let menu = Submenu::with_id(app, "popup-menu", tr(app, "Popup"), true)?;
    menu.append(&check(app, ID_POPUP, &tr(app, "Mostrar ao trocar de faixa"), settings::popup_enabled(app))?)?;
    menu.append(&check(app, ID_POPUP_PLAY_PAUSE, &tr(app, "Mostrar também ao pausar/tocar"), settings::popup_on_play_pause(app))?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&position)?;
    menu.append(&duration)?;
    Ok(menu)
}

fn panel_menu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let current = windows::target_monitor(app).map(|m| windows::monitor_ref(&m));
    let monitors = Submenu::with_id(app, "monitors", tr(app, "Monitor"), true)?;
    for (i, m) in windows::monitors(app).iter().enumerate() {
        let r = windows::monitor_ref(m);
        let primary = if windows::is_primary(app, m) { tr(app, " (principal)") } else { String::new() };
        let label = fmt(
            app,
            "Monitor {} — {}×{}{}",
            &[&(i + 1).to_string(), &m.size().width.to_string(), &m.size().height.to_string(), &primary],
        );
        // O id carrega a posição e o nome: sobrevive a mudanças na lista.
        let id = format!("{PREFIX_MONITOR}{}:{}:{}", r.x, r.y, r.name.clone().unwrap_or_default());
        monitors.append(&check(app, &id, &label, current.as_ref() == Some(&r))?)?;
    }

    let menu = Submenu::with_id(app, "panel-menu", tr(app, "Painel"), true)?;
    menu.append(&check(app, ID_PANEL, &tr(app, "Mostrar painel"), settings::panel_enabled(app))?)?;
    menu.append(&monitors)?;
    // Desligado = janela livre: arraste pela faixa de cima e redimensione pelas bordas.
    menu.append(&check(app, ID_FILL, &tr(app, "Ocupar o monitor inteiro"), settings::panel_fills_monitor(app))?)?;
    Ok(menu)
}

/// "Fonte de mídia": todos os apps, ou só um (ex.: YouTube Music). Evita que vídeos
/// do navegador tomem o lugar quando o player principal pausa.
fn source_menu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let chosen = settings::media_source(app);
    let menu = Submenu::with_id(app, "sources", tr(app, "Fonte de mídia"), true)?;
    menu.append(&check(app, PREFIX_SOURCE, &tr(app, "Todos os apps"), chosen.is_none())?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    // Os apps mais comuns aparecem sempre, mesmo fechados: dá para escolher antes de abrir o player.
    for (part, name) in KNOWN_SOURCES {
        let on = chosen.as_deref().is_some_and(|c| media::source_matches(&format!("~{part}"), c) || c == format!("~{part}"));
        menu.append(&check(app, &format!("{PREFIX_SOURCE}~{part}"), &fmt(app, "Só {}", &[name]), on)?)?;
    }
    // Os outros apps com sessão aberta agora.
    let is_known = |id: &str| KNOWN_SOURCES.iter().any(|(part, _)| media::source_matches(&format!("~{part}"), id));
    let mut listed = Vec::new();
    for s in media::sources(app) {
        if is_known(&s.id) {
            continue;
        }
        let id = format!("{PREFIX_SOURCE}{}", s.id);
        menu.append(&check(app, &id, &fmt(app, "Só {}", &[&s.label]), chosen.as_deref() == Some(s.id.as_str()))?)?;
        listed.push(s.id);
    }
    // A fonte escolhida pode estar fechada agora: continua marcada, para dar para desmarcar.
    if let Some(id) = chosen.filter(|c| !listed.contains(c) && !c.starts_with('~') && !is_known(c)) {
        let label = fmt(app, "Só {} (fechado)", &[&media::friendly_name(&id)]);
        menu.append(&check(app, &format!("{PREFIX_SOURCE}{id}"), &label, true)?)?;
    }
    Ok(menu)
}

/// "Estante": guardar o que você ouve (só neste computador) e apagar o histórico.
fn shelf_menu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let menu = Submenu::with_id(app, "shelf", tr(app, "Estante"), true)?;
    menu.append(&check(app, ID_HISTORY, &tr(app, "Guardar o que eu ouço (só neste computador)"), settings::history_enabled(app))?)?;
    menu.append(&MenuItem::with_id(app, ID_CLEAR_HISTORY, tr(app, "Limpar a estante (apaga o histórico)"), true, None::<&str>)?)?;
    Ok(menu)
}

/// "Efeitos do disco": intensidade do brilho que gira, dos riscos e do tremor do braço.
fn fx_menu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let current = settings::disk_fx(app);
    let menu = Submenu::with_id(app, "fx", tr(app, "Efeitos do disco"), true)?;
    for (value, label) in [("off", "Desligados"), ("soft", "Suaves"), ("strong", "Fortes")] {
        menu.append(&check(app, &format!("{PREFIX_FX}{value}"), &tr(app, label), current == value)?)?;
    }
    Ok(menu)
}

/// "Dormir": pausa a música depois de um tempo.
fn sleep_menu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let left = sleep::remaining_min(app);
    let title = match left {
        Some(m) => fmt(app, "Dormir — pausa em {} min", &[&m.to_string()]),
        None => tr(app, "Dormir (pausar depois de...)"),
    };
    let menu = Submenu::with_id(app, "sleep", title, true)?;
    for m in sleep::OPTIONS_MIN {
        menu.append(&MenuItem::with_id(app, format!("{PREFIX_SLEEP}{m}"), fmt(app, "{} minutos", &[&m.to_string()]), true, None::<&str>)?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, ID_SLEEP_CANCEL, tr(app, "Cancelar o timer"), left.is_some(), None::<&str>)?)?;
    Ok(menu)
}

/// "Vinil": velocidade do giro (33/45 rpm) e chiado de agulha sobre a música.
fn vinyl_menu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let rpm = Submenu::with_id(app, "vinyl-rpm", tr(app, "Rotação do disco"), true)?;
    let current_rpm = settings::vinyl_rpm(app);
    for r in settings::VINYL_RPMS {
        rpm.append(&check(app, &format!("{PREFIX_RPM}{r}"), &format!("{r} rpm"), current_rpm == r)?)?;
    }
    let crackle = Submenu::with_id(app, "vinyl-crackle", tr(app, "Som de disco antigo (com o painel aberto)"), true)?;
    let current = settings::crackle(app);
    for (value, label) in [("off", "Desligado"), ("soft", "Leve"), ("medium", "Médio"), ("strong", "Gasto (muito velho)")] {
        crackle.append(&check(app, &format!("{PREFIX_CRACKLE}{value}"), &tr(app, label), current == value)?)?;
    }
    let menu = Submenu::with_id(app, "vinyl", tr(app, "Vinil"), true)?;
    menu.append(&rpm)?;
    menu.append(&crackle)?;
    Ok(menu)
}

/// "Idioma": automático (o do Windows), português ou inglês.
fn language_menu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let current = settings::language(app);
    let menu = Submenu::with_id(app, "language", tr(app, "Idioma"), true)?;
    let choices = [("auto", tr(app, "Automático (idioma do Windows)")), ("pt", "Português".to_string()), ("en", "English".to_string())];
    for (value, label) in choices {
        menu.append(&check(app, &format!("{PREFIX_LANG}{value}"), &label, current == value)?)?;
    }
    Ok(menu)
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let dynamic = check(app, ID_DYNAMIC_COLOR, &tr(app, "Cor de destaque da capa"), settings::dynamic_color(app))?;
    let lyrics = check(app, ID_LYRICS, &tr(app, "Letras sincronizadas (consulta lrclib.net)"), settings::lyrics_enabled(app))?;
    let hd_cover = check(app, ID_HD_COVER, &tr(app, "Capa em alta resolução (consulta iTunes)"), settings::hd_cover(app))?;
    let discord = check(app, ID_DISCORD, &tr(app, "Call do Discord na tela cheia (precisa de configuração)"), settings::discord_enabled(app))?;
    // Precisa do Client ID (o mesmo da call); sem ele a opção fica apagada.
    let has_id = settings::discord_credentials(app).is_some();
    let presence = CheckMenuItem::with_id(
        app,
        ID_PRESENCE,
        &tr(app, if has_id { "Mostrar no Discord o que estou ouvindo" } else { "Mostrar no Discord o que estou ouvindo (configure o Discord antes)" }),
        has_id,
        settings::discord_presence(app),
        None::<&str>,
    )?;
    let overlay = check(
        app,
        ID_OVERLAY,
        &fmt(app, "Overlay para OBS (http://localhost:{})", &[&crate::overlay::PORT.to_string()]),
        settings::overlay_enabled(app),
    )?;
    let shortcuts = check(
        app,
        ID_SHORTCUTS,
        &fmt(app, "Atalhos globais ({})", &[shortcuts::SUMMARY]),
        settings::shortcuts_enabled(app),
    )?;
    let autostart = check(app, ID_AUTOSTART, &tr(app, "Iniciar com o Windows"), app.autolaunch().is_enabled().unwrap_or(false))?;
    let logs = MenuItem::with_id(app, ID_LOGS, tr(app, "Abrir a pasta de logs (para relatar um problema)"), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, ID_QUIT, tr(app, "Sair"), true, None::<&str>)?;
    Menu::with_items(
        app,
        &[
            &source_menu(app)?,
            &popup_menu(app)?,
            &panel_menu(app)?,
            &fx_menu(app)?,
            &vinyl_menu(app)?,
            &sleep_menu(app)?,
            &shelf_menu(app)?,
            &dynamic,
            &lyrics,
            &hd_cover,
            &discord,
            &presence,
            &overlay,
            &shortcuts,
            &autostart,
            &language_menu(app)?,
            &PredefinedMenuItem::separator(app)?,
            &logs,
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
        ID_PRESENCE => {
            settings::set_discord_presence(app, !settings::discord_presence(app));
            crate::presence::update(app); // mostra (ou limpa) na hora
        }
        ID_OVERLAY => {
            settings::set_overlay_enabled(app, !settings::overlay_enabled(app));
            crate::overlay::reload(app); // liga/desliga o servidor na hora
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
        ID_SLEEP_CANCEL => sleep::cancel(app),
        ID_LOGS => crate::logfile::open_folder(app),
        ID_QUIT => app.exit(0),
        _ => {
            if let Some(source) = id.strip_prefix(PREFIX_SOURCE) {
                // "source:" vazio = todos os apps
                media::set_source(app, (!source.is_empty()).then(|| source.to_string()));
            } else if let Some(level) = id.strip_prefix(PREFIX_FX).filter(|l| settings::DISK_FX_LEVELS.contains(l)) {
                settings::set_disk_fx(app, level);
                let _ = app.emit("disk-fx", level); // painel e popup aplicam na hora
            } else if let Some(rpm) = id.strip_prefix(PREFIX_RPM).and_then(|v| v.parse().ok()).filter(|r| settings::VINYL_RPMS.contains(r)) {
                settings::set_vinyl_rpm(app, rpm);
                let _ = app.emit("vinyl", settings::vinyl(app));
            } else if let Some(level) = id.strip_prefix(PREFIX_CRACKLE).filter(|l| settings::CRACKLE_LEVELS.contains(l)) {
                settings::set_crackle(app, level);
                let _ = app.emit("vinyl", settings::vinyl(app));
            } else if let Some(min) = id.strip_prefix(PREFIX_SLEEP).and_then(|v| v.parse().ok()).filter(|m| sleep::OPTIONS_MIN.contains(m)) {
                sleep::start(app, min);
            } else if let Some(choice) = id.strip_prefix(PREFIX_LANG).filter(|c| ["auto", "pt", "en"].contains(c)) {
                i18n::set_language(app, choice);
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
