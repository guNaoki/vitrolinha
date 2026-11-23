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

/// Rotação do disco: 16 (⅔, discos antigos), 33 (padrão) ou 45 rpm. Muda o giro na tela e o
/// compasso do "tec" do som de disco antigo.
pub const VINYL_RPMS: [u32; 3] = [16, 33, 45];
pub fn vinyl_rpm(app: &AppHandle) -> u32 {
    get::<u32>(app, "vinylRpm").filter(|v| VINYL_RPMS.contains(v)).unwrap_or(33)
}
pub fn set_vinyl_rpm(app: &AppHandle, rpm: u32) {
    set(app, "vinylRpm", json!(rpm));
}

/// Som de disco antigo sobre a música (só com o painel aberto): "off" (padrão) | "soft" | "medium" | "strong".
pub const CRACKLE_LEVELS: [&str; 4] = ["off", "soft", "medium", "strong"];
pub fn crackle(app: &AppHandle) -> String {
    get::<String>(app, "crackle")
        .filter(|v| CRACKLE_LEVELS.contains(&v.as_str()))
        .unwrap_or_else(|| "off".into())
}
pub fn set_crackle(app: &AppHandle, level: &str) {
    set(app, "crackle", json!(level));
}

/// O que o front precisa saber do "vinil" (evento `vinyl` e comando `get_vinyl`).
#[derive(Clone, Serialize)]
pub struct Vinyl {
    pub rpm: u32,
    pub crackle: String,
}
pub fn vinyl(app: &AppHandle) -> Vinyl {
    Vinyl { rpm: vinyl_rpm(app), crackle: crackle(app) }
}

/// Overlay para OBS em http://localhost:7421 (padrão: não).
pub fn overlay_enabled(app: &AppHandle) -> bool {
    get(app, "overlayEnabled").unwrap_or(false)
}
pub fn set_overlay_enabled(app: &AppHandle, on: bool) {
    set(app, "overlayEnabled", json!(on));
}

/// Idioma da interface: "auto" (o do Windows, padrão) | "pt" | "en".
pub fn language(app: &AppHandle) -> String {
    get::<String>(app, "language")
        .filter(|v| ["auto", "pt", "en"].contains(&v.as_str()))
        .unwrap_or_else(|| "auto".into())
}
pub fn set_language(app: &AppHandle, choice: &str) {
    set(app, "language", json!(choice));
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

/// Cor de destaque escolhida à mão ("#rrggbb", minúsculas). Sem ela vale `dynamic_color`:
/// da capa (padrão) ou o âmbar clássico.
pub fn valid_hex(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].bytes().all(|b| b.is_ascii_hexdigit())
}
pub fn accent_custom(app: &AppHandle) -> Option<String> {
    get::<String>(app, "accentCustom").filter(|c| valid_hex(c)).map(|c| c.to_ascii_lowercase())
}
pub fn set_accent_custom(app: &AppHandle, color: Option<&str>) {
    set(app, "accentCustom", json!(color));
}

/// "Atmosfera" do painel: brilho da cor no fundo e granulado de filme, ambos de 0 a 100.
#[derive(Clone, Copy, Serialize)]
pub struct Look {
    pub glow: u8,
    pub grain: u8,
}
pub const DEFAULT_LOOK: Look = Look { glow: 25, grain: 33 };
pub fn look(app: &AppHandle) -> Look {
    let pct = |key: &str, default: u8| get::<u8>(app, key).map(|v| v.min(100)).unwrap_or(default);
    Look { glow: pct("lookGlow", DEFAULT_LOOK.glow), grain: pct("lookGrain", DEFAULT_LOOK.grain) }
}
pub fn set_look(app: &AppHandle, glow: u8, grain: u8) {
    set(app, "lookGlow", json!(glow.min(100)));
    set(app, "lookGrain", json!(grain.min(100)));
}

// ---- Discord: arquivo próprio (discord.json)
// Fica fora do settings.json de propósito: aquele é regravado pelo app a partir de uma cópia
// em memória (editar com o app aberto perde a edição; um BOM invalida o arquivo todo).
// Este é relido do disco a cada uso, aceita BOM e só é gravado por leitura-modificação-escrita
// (arquivo temporário + renomear), então nada além de uma gravação deliberada o altera.
// Chaves: `enabled`, `clientId`, `clientSecret`, `redirectUri`, `token`. Se `discord.json`
// não tem a chave, vale o que houver no settings.json (versão antiga).

fn discord_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    use tauri::Manager;
    app.path().app_data_dir().ok().map(|d| d.join("discord.json"))
}

fn discord_read(app: &AppHandle) -> serde_json::Map<String, Value> {
    let Some(path) = discord_file(app) else { return Default::default() };
    let Ok(bytes) = std::fs::read(&path) else { return Default::default() };
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&bytes);
    match serde_json::from_slice(bytes) {
        Ok(Value::Object(map)) => map,
        _ => {
            eprintln!("[settings] discord.json inválido; ignorando");
            Default::default()
        }
    }
}

fn discord_get<T: DeserializeOwned>(app: &AppHandle, key: &str, old_key: &str) -> Option<T> {
    discord_read(app)
        .remove(key)
        .and_then(|v| serde_json::from_value(v).ok())
        .or_else(|| get(app, old_key))
}

fn discord_set(app: &AppHandle, key: &str, value: Value) {
    let Some(path) = discord_file(app) else { return };
    let mut map = discord_read(app);
    if value.is_null() {
        map.remove(key);
    } else {
        map.insert(key.to_string(), value);
    }
    let tmp = path.with_extension("json.tmp");
    let result = serde_json::to_vec_pretty(&Value::Object(map))
        .map_err(|e| e.to_string())
        .and_then(|bytes| {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
            std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
        });
    if let Err(e) = result {
        eprintln!("[settings] falha ao gravar discord.json: {e}");
    }
}

/// Lista da call de voz do Discord na tela cheia (padrão: não — precisa de um app próprio
/// no Discord Developer Portal; ver README e scripts/discord-setup.ps1).
pub fn discord_enabled(app: &AppHandle) -> bool {
    discord_get(app, "enabled", "discordEnabled").unwrap_or(false)
}
pub fn set_discord_enabled(app: &AppHandle, on: bool) {
    discord_set(app, "enabled", json!(on));
}
/// "Ouvindo agora" no perfil do Discord (padrão: não). Precisa do mesmo Client ID da call.
pub fn discord_presence(app: &AppHandle) -> bool {
    discord_get(app, "presence", "discordPresence").unwrap_or(false)
}
pub fn set_discord_presence(app: &AppHandle, on: bool) {
    discord_set(app, "presence", json!(on));
}
/// Client ID do app do Discord que acompanha o programa (cliente público, sem secret: o
/// Client ID não é segredo). Vazio até o app público ser criado no Developer Portal; enquanto
/// isso, cada pessoa usa o `clientId` do próprio discord.json.
const BUILT_IN_DISCORD_CLIENT_ID: &str = "";

/// (client id, client secret opcional) do app do Discord. Sem secret, a autorização usa PKCE
/// (cliente público); com secret, o fluxo clássico. `None` se não houver client id nenhum.
pub fn discord_credentials(app: &AppHandle) -> Option<(String, Option<String>)> {
    let own_id = discord_get::<String>(app, "clientId", "discordClientId").map(|s| s.trim().to_string());
    let id = own_id.filter(|s| !s.is_empty()).unwrap_or_else(|| BUILT_IN_DISCORD_CLIENT_ID.to_string());
    if id.is_empty() {
        return None;
    }
    let secret = discord_get::<String>(app, "clientSecret", "discordClientSecret")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    Some((id, secret))
}
/// Redirect cadastrado no app (aba OAuth2); o padrão é o que o README manda cadastrar.
pub fn discord_redirect_uri(app: &AppHandle) -> String {
    discord_get::<String>(app, "redirectUri", "discordRedirectUri")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://localhost".to_string())
}
/// Token do Discord guardado após a 1ª autorização (para não pedir de novo).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscordToken {
    pub access: String,
    pub refresh: Option<String>,
}
pub fn discord_token(app: &AppHandle) -> Option<DiscordToken> {
    discord_get(app, "token", "discordToken")
}
pub fn set_discord_token(app: &AppHandle, token: Option<&DiscordToken>) {
    discord_set(app, "token", json!(token));
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

#[cfg(test)]
mod tests {
    use super::valid_hex;

    #[test]
    fn only_full_hex_colors_are_accepted() {
        assert!(valid_hex("#e5484d"));
        assert!(valid_hex("#ABCDEF"));
        assert!(!valid_hex("e5484d"));
        assert!(!valid_hex("#e5484"));
        assert!(!valid_hex("#e5484dd"));
        assert!(!valid_hex("#e5484g"));
        assert!(!valid_hex("cover"));
    }
}
