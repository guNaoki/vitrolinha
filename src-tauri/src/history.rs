//! Estante: os últimos discos que você ouviu, guardados SÓ neste computador.
//!
//! Cada faixa vira uma entrada (título, artista, álbum, quantas vezes, quando) e uma
//! miniatura da capa de 320 px. Fica em `%APPDATA%\app.vitrolinha\shelf\`
//! (`history.json` + `covers\`). Nada é enviado para fora. Pode ser desligada e
//! apagada pelo menu da bandeja.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::media::NowPlaying;
use crate::{artwork, settings};

/// Quantos discos a estante guarda (os mais recentes).
const LIMIT: usize = 60;
/// Lado menor das miniaturas (px).
const THUMB: u32 = 320;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    key: String,
    title: String,
    artist: String,
    album: String,
    /// Nome do arquivo da miniatura em `covers/` (se já houve capa).
    cover: Option<String>,
    /// Tamanho da imagem de origem da miniatura: só troca por uma origem maior (capa HD).
    cover_source_bytes: u64,
    plays: u32,
    last_ms: i64,
}

/// O que o front recebe (espelha `ShelfRecord` em src/lib/shelf.ts).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    title: String,
    artist: String,
    album: String,
    cover_path: Option<String>,
    plays: u32,
    last_ms: i64,
}

#[derive(Default)]
pub struct HistoryState {
    /// Carregado do disco na primeira vez que precisa.
    entries: Mutex<Option<Vec<Entry>>>,
    /// Faixa já contada (não conta de novo a cada atualização do player).
    last_key: Mutex<String>,
    /// Última capa oferecida (evita refazer a miniatura à toa).
    last_art: Mutex<String>,
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn dir(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_data_dir().ok()?.join("shelf"))
}

fn key_of(title: &str, artist: &str) -> String {
    format!("{}|{}", title.trim().to_lowercase(), artist.trim().to_lowercase())
}

/// Nome de arquivo estável a partir da chave (FNV-1a).
fn file_name(key: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}.jpg")
}

fn load(app: &AppHandle) -> Vec<Entry> {
    let Some(path) = dir(app).map(|d| d.join("history.json")) else { return Vec::new() };
    fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save(app: &AppHandle, entries: &[Entry]) {
    let Some(dir) = dir(app) else { return };
    let _ = fs::create_dir_all(&dir);
    let tmp = dir.join("history.json.tmp");
    let target = dir.join("history.json");
    match serde_json::to_string(entries) {
        Ok(json) => {
            if fs::write(&tmp, json).and_then(|_| fs::rename(&tmp, &target)).is_err() {
                eprintln!("[estante] falha ao salvar o histórico");
            }
        }
        Err(e) => eprintln!("[estante] falha ao serializar: {e}"),
    }
}

/// Chamado a cada estado publicado pelo player: conta a faixa nova (só tocando) e
/// guarda a capa dela (a primeira miniatura, e depois uma melhor, se vier).
pub fn observe(app: &AppHandle, np: &NowPlaying) {
    if np.title.is_empty() || !settings::history_enabled(app) {
        return;
    }
    let state = app.state::<HistoryState>();
    let key = key_of(&np.title, &np.artist);

    let mut guard = state.entries.lock().unwrap();
    let entries = guard.get_or_insert_with(|| load(app));
    let mut dirty = false;

    if np.status == "playing" {
        let mut last = state.last_key.lock().unwrap();
        if *last != key {
            *last = key.clone();
            let mut entry = match entries.iter().position(|e| e.key == key) {
                Some(i) => entries.remove(i),
                None => Entry {
                    key: key.clone(),
                    title: np.title.clone(),
                    artist: np.artist.clone(),
                    album: np.album.clone(),
                    cover: None,
                    cover_source_bytes: 0,
                    plays: 0,
                    last_ms: 0,
                },
            };
            entry.plays += 1;
            entry.last_ms = now_ms();
            if entry.album.is_empty() {
                entry.album = np.album.clone();
            }
            entries.insert(0, entry);
            // Passou do limite: descarta o mais antigo e a miniatura dele.
            while entries.len() > LIMIT {
                if let Some(old) = entries.pop() {
                    remove_cover(app, &old);
                }
            }
            dirty = true;
        }
    }

    if let Some(path) = &np.artwork_path {
        dirty |= offer_cover_locked(app, &state, entries, &key, path);
    }
    if dirty {
        save(app, entries);
    }
}

/// Capa nova de uma faixa já conhecida (ex.: a de alta resolução, que chega depois).
pub fn offer_cover(app: &AppHandle, np: &NowPlaying, path: &str) {
    if !settings::history_enabled(app) {
        return;
    }
    let state = app.state::<HistoryState>();
    let key = key_of(&np.title, &np.artist);
    let mut guard = state.entries.lock().unwrap();
    let entries = guard.get_or_insert_with(|| load(app));
    if offer_cover_locked(app, &state, entries, &key, path) {
        save(app, entries);
    }
}

fn offer_cover_locked(app: &AppHandle, state: &HistoryState, entries: &mut [Entry], key: &str, path: &str) -> bool {
    {
        let mut last = state.last_art.lock().unwrap();
        let marker = format!("{key}|{path}");
        if *last == marker {
            return false;
        }
        *last = marker;
    }
    let Some(entry) = entries.iter_mut().find(|e| e.key == key) else { return false };
    let Ok(bytes) = fs::read(path) else { return false };
    let source = bytes.len() as u64;
    if entry.cover.is_some() && source <= entry.cover_source_bytes {
        return false; // já temos uma origem igual ou maior
    }
    let Some(thumb) = artwork::shrink_jpeg(&bytes, THUMB) else { return false };
    let Some(dir) = dir(app) else { return false };
    let covers = dir.join("covers");
    let _ = fs::create_dir_all(&covers);
    let name = file_name(key);
    if fs::write(covers.join(&name), thumb).is_err() {
        return false;
    }
    entry.cover = Some(name);
    entry.cover_source_bytes = source;
    true
}

fn remove_cover(app: &AppHandle, e: &Entry) {
    if let (Some(dir), Some(name)) = (dir(app), &e.cover) {
        let _ = fs::remove_file(dir.join("covers").join(name));
    }
}

/// A estante, do disco mais recente para o mais antigo.
pub fn list(app: &AppHandle) -> Vec<Record> {
    let state = app.state::<HistoryState>();
    let mut guard = state.entries.lock().unwrap();
    let entries = guard.get_or_insert_with(|| load(app));
    let covers = dir(app).map(|d| d.join("covers"));
    entries
        .iter()
        .map(|e| Record {
            title: e.title.clone(),
            artist: e.artist.clone(),
            album: e.album.clone(),
            cover_path: match (&covers, &e.cover) {
                (Some(c), Some(name)) => Some(c.join(name).to_string_lossy().into_owned()),
                _ => None,
            },
            plays: e.plays,
            last_ms: e.last_ms,
        })
        .collect()
}

/// Apaga o histórico e as miniaturas.
pub fn clear(app: &AppHandle) {
    let state = app.state::<HistoryState>();
    *state.entries.lock().unwrap() = Some(Vec::new());
    state.last_key.lock().unwrap().clear();
    state.last_art.lock().unwrap().clear();
    if let Some(dir) = dir(app) {
        let _ = fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_ignore_case_and_spaces() {
        assert_eq!(key_of(" Sunflower ", "Peach Luffe"), key_of("sunflower", "peach luffe "));
        assert_ne!(key_of("Sunflower", "A"), key_of("Sunflower", "B"));
    }

    #[test]
    fn file_names_are_stable_and_distinct() {
        assert_eq!(file_name("a|b"), file_name("a|b"));
        assert_ne!(file_name("a|b"), file_name("a|c"));
        assert!(file_name("a|b").ends_with(".jpg"));
    }
}
