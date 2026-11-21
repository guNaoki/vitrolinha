//! Persistência (plugin store → settings.json na pasta de dados do app).

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const FILE: &str = "settings.json";

/// Monitor escolhido para o painel: nome + posição (o nome sozinho pode
/// mudar quando os monitores são reconectados).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MonitorRef {
    pub name: Option<String>,
    pub x: i32,
    pub y: i32,
}

fn get<T: DeserializeOwned>(app: &AppHandle, key: &str) -> Option<T> {
    let value = app.store(FILE).ok()?.get(key)?;
    serde_json::from_value(value).ok()
}

fn set(app: &AppHandle, key: &str, value: Value) {
    match app.store(FILE) {
        Ok(store) => {
            store.set(key, value);
            if let Err(e) = store.save() {
                eprintln!("[settings] falha ao salvar: {e}");
            }
        }
        Err(e) => eprintln!("[settings] falha ao abrir o store: {e}"),
    }
}

/// Painel ligado? (padrão: sim)
pub fn panel_enabled(app: &AppHandle) -> bool {
    get(app, "panelEnabled").unwrap_or(true)
}
pub fn set_panel_enabled(app: &AppHandle, on: bool) {
    set(app, "panelEnabled", json!(on));
}

/// Painel ocupa o monitor inteiro (padrão) ou usa janela de tamanho fixo.
pub fn panel_fills_monitor(app: &AppHandle) -> bool {
    get(app, "panelFillsMonitor").unwrap_or(true)
}
pub fn set_panel_fills_monitor(app: &AppHandle, on: bool) {
    set(app, "panelFillsMonitor", json!(on));
}

/// Atalhos globais ligados? (padrão: sim)
pub fn shortcuts_enabled(app: &AppHandle) -> bool {
    get(app, "shortcutsEnabled").unwrap_or(true)
}
pub fn set_shortcuts_enabled(app: &AppHandle, on: bool) {
    set(app, "shortcutsEnabled", json!(on));
}
/// Combinação de um atalho (editável no settings.json).
pub fn shortcut(app: &AppHandle, key: &str, default: &str) -> String {
    get(app, key).unwrap_or_else(|| default.to_string())
}

/// Letras sincronizadas ligadas? Consulta o lrclib.net com título/artista/álbum/duração
/// da faixa. (padrão: sim)
pub fn lyrics_enabled(app: &AppHandle) -> bool {
    get(app, "lyricsEnabled").unwrap_or(true)
}
pub fn set_lyrics_enabled(app: &AppHandle, on: bool) {
    set(app, "lyricsEnabled", json!(on));
}

/// Guarda no computador o que você ouve, para a estante (padrão: sim). Nada sai da máquina.
pub fn history_enabled(app: &AppHandle) -> bool {
    get(app, "historyEnabled").unwrap_or(true)
}
pub fn set_history_enabled(app: &AppHandle, on: bool) {
    set(app, "historyEnabled", json!(on));
}

/// Efeitos do disco (brilho que gira, riscos, tremor do braço): "off" | "soft" | "strong".
/// (padrão: soft)
pub const DISK_FX_LEVELS: [&str; 3] = ["off", "soft", "strong"];
pub fn disk_fx(app: &AppHandle) -> String {
    get::<String>(app, "diskFx")
        .filter(|v| DISK_FX_LEVELS.contains(&v.as_str()))
        .unwrap_or_else(|| "soft".into())
}
pub fn set_disk_fx(app: &AppHandle, level: &str) {
    set(app, "diskFx", json!(level));
}

/// Busca a capa em alta resolução no iTunes quando a do player é pequena (padrão: sim).
pub fn hd_cover(app: &AppHandle) -> bool {
    get(app, "hdCover").unwrap_or(true)
}
pub fn set_hd_cover(app: &AppHandle, on: bool) {
    set(app, "hdCover", json!(on));
}

/// Acento do painel/popup segue a capa da faixa (padrão: sim).
pub fn dynamic_color(app: &AppHandle) -> bool {
    get(app, "dynamicColor").unwrap_or(true)
}
pub fn set_dynamic_color(app: &AppHandle, on: bool) {
    set(app, "dynamicColor", json!(on));
}

/// Popup ligado? (padrão: sim)
pub fn popup_enabled(app: &AppHandle) -> bool {
    get(app, "popupEnabled").unwrap_or(true)
}
pub fn set_popup_enabled(app: &AppHandle, on: bool) {
    set(app, "popupEnabled", json!(on));
}

/// Quanto tempo o popup fica na tela (ms). Padrão 4000.
pub fn popup_duration_ms(app: &AppHandle) -> u64 {
    get::<u64>(app, "popupDurationMs").unwrap_or(4_000).clamp(1_000, 30_000)
}

/// "bottom-center" (padrão), "top-left", "bottom-right"… Escolhida pelo menu da bandeja.
pub fn popup_position(app: &AppHandle) -> String {
    get(app, "popupPosition").unwrap_or_else(|| "bottom-center".to_string())
}

/// Mostrar também ao dar play/pause (padrão: não).
pub fn popup_on_play_pause(app: &AppHandle) -> bool {
    get(app, "popupOnPlayPause").unwrap_or(false)
}

pub fn set_popup_duration_ms(app: &AppHandle, ms: u64) {
    set(app, "popupDurationMs", json!(ms));
}
pub fn set_popup_position(app: &AppHandle, position: &str) {
    set(app, "popupPosition", json!(position));
}
pub fn set_popup_on_play_pause(app: &AppHandle, on: bool) {
    set(app, "popupOnPlayPause", json!(on));
}

/// Posição e tamanho (pixels físicos) da janela livre.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RectRef {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}
pub fn panel_rect(app: &AppHandle) -> Option<RectRef> {
    get(app, "panelRect")
}
pub fn set_panel_rect(app: &AppHandle, r: &RectRef) {
    set(app, "panelRect", json!(r));
}

/// App de mídia escolhido como única fonte (AUMID); `None` = qualquer app.
pub fn media_source(app: &AppHandle) -> Option<String> {
    get(app, "mediaSource")
}
pub fn set_media_source(app: &AppHandle, source: Option<&str>) {
    set(app, "mediaSource", json!(source));
}

pub fn panel_monitor(app: &AppHandle) -> Option<MonitorRef> {
    get(app, "panelMonitor")
}
pub fn set_panel_monitor(app: &AppHandle, m: &MonitorRef) {
    set(app, "panelMonitor", json!(m));
}
