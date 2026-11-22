//! Capa em alta resolução.
//!
//! O player (principalmente o YouTube Music no navegador) costuma entregar ao Windows
//! um thumbnail minúsculo, que fica borrado no selo do disco. Para faixas de catálogo,
//! busca a capa grande no iTunes Search API.
//!
//! Privacidade: a busca envia ao itunes.apple.com só artista e título da faixa que está
//! tocando (e um User-Agent "Vitrolinha" com o endereço do projeto). Pode ser desligada no menu da bandeja.
//! Sem resultado confiável (nome e duração parecidos), mantém a capa do player.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::thread;

use serde_json::Value;
use tauri::{AppHandle, Manager};
use windows::core::HSTRING;
use windows::Foundation::Uri;
use windows::Storage::Streams::DataReader;
use windows::Web::Http::HttpClient;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

use crate::lyrics::clean_title;
use crate::media::{self, NowPlaying};
use crate::settings;

const USER_AGENT: &str = "Vitrolinha/0.1 (+https://github.com/guNaoki/vitrolinha)";
/// Lado (px) da capa pedida. O iTunes entrega até ~3000; 1000 sobra para o selo.
const SIZE: &str = "1000x1000bb";
/// Capas do player com lado menor a partir disso já estão boas: não busca.
pub const GOOD_ENOUGH: u32 = 700;
/// Duração da faixa × a do resultado: acima disso é outra versão.
const MAX_DURATION_DIFF_MS: i64 = 8_000;

struct Hd {
    key: String,
    path: String,
}

#[derive(Default)]
pub struct HdState {
    hd: Mutex<Option<Hd>>,
}

fn key_of(np: &NowPlaying) -> String {
    format!("{}|{}|{}|{}", np.title, np.artist, np.album, np.source_app)
}

/// Troca o caminho da capa pela de alta resolução, se houver uma para esta faixa
/// e a opção estiver ligada. Chamado por `media::for_ui` em todo estado que vai ao front.
pub fn apply(app: &AppHandle, np: &mut NowPlaying) {
    if !settings::hd_cover(app) {
        return;
    }
    if let Some(hd) = app.state::<HdState>().hd.lock().unwrap().as_ref() {
        if hd.key == key_of(np) {
            np.artwork_path = Some(hd.path.clone());
        }
    }
}

/// Faixa nova: procura a capa grande, se a do player é pequena (ou não existe).
/// `short_side` = lado menor do thumbnail do player (0 = sem capa).
pub fn request(app: &AppHandle, np: &NowPlaying, short_side: u32) {
    if np.title.is_empty() || !settings::hd_cover(app) || short_side >= GOOD_ENOUGH {
        return;
    }
    let (app, np) = (app.clone(), np.clone());
    thread::spawn(move || {
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let url = match find(&np) {
            Ok(Some(u)) => u,
            Ok(None) => return,
            Err(e) => {
                eprintln!("[capa-hd] falha na busca: {e}");
                return;
            }
        };
        let bytes = match download(&url) {
            Ok(b) if b.len() > 1_000 => b,
            Ok(_) => return,
            Err(e) => {
                eprintln!("[capa-hd] falha ao baixar: {e}");
                return;
            }
        };
        // A faixa já mudou enquanto baixava? Descarta.
        let still_current = app
            .state::<media::SharedState>()
            .now
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|cur| key_of(cur) == key_of(&np));
        if !still_current {
            return;
        }
        let Some(path) = save(&bytes) else { return };
        crate::history::offer_cover(&app, &np, &path.to_string_lossy()); // a estante também ganha a capa grande
        println!("[capa-hd] {} — {} ({} KB)", np.title, np.artist, bytes.len() / 1024);
        let old = app
            .state::<HdState>()
            .hd
            .lock()
            .unwrap()
            .replace(Hd { key: key_of(&np), path: path.to_string_lossy().into_owned() });
        if let Some(old) = old {
            let _ = fs::remove_file(old.path);
        }
        media::refresh(&app); // reemite com a capa nova
    });
}

/// Liga/desliga a opção: aplica na faixa atual.
pub fn reload(app: &AppHandle) {
    let current = app.state::<media::SharedState>().now.lock().unwrap().clone();
    match current {
        Some(np) if settings::hd_cover(app) => {
            let have = app.state::<HdState>().hd.lock().unwrap().as_ref().is_some_and(|h| h.key == key_of(&np));
            if !have {
                request(app, &np, 0);
            }
            media::refresh(app);
        }
        _ => media::refresh(app),
    }
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn save(bytes: &[u8]) -> Option<PathBuf> {
    let dir = std::env::temp_dir().join("vitrolinha-art");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join(format!("hd-{}.jpg", COUNTER.fetch_add(1, Ordering::SeqCst) + 1));
    fs::write(&path, bytes).ok()?;
    Some(path)
}

fn client() -> windows::core::Result<HttpClient> {
    let client = HttpClient::new()?;
    client.DefaultRequestHeaders()?.UserAgent()?.TryParseAdd(&HSTRING::from(USER_AGENT))?;
    Ok(client)
}

fn encode(s: &str) -> Option<String> {
    Some(Uri::EscapeComponent(&HSTRING::from(s)).ok()?.to_string())
}

/// URL da capa grande do melhor resultado confiável, se houver.
fn find(np: &NowPlaying) -> Result<Option<String>, String> {
    let client = client().map_err(|e| e.to_string())?;
    let term = format!("{} {}", np.artist, clean_title(&np.title));
    let url = format!(
        "https://itunes.apple.com/search?media=music&entity=song&limit=10&term={}",
        encode(&term).ok_or("termo inválido")?
    );
    let uri = Uri::CreateUri(&HSTRING::from(url)).map_err(|e| e.to_string())?;
    let body = client
        .GetStringAsync(&uri)
        .and_then(|op| op.join())
        .map_err(|e| e.to_string())?
        .to_string();
    let json: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let results = json.get("results").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(pick(&results, &np.title, &np.artist, np.duration_ms))
}

/// Só letras/números, minúsculas ("Peach Luffe" → "peachluffe").
fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Resultado cujo título e artista batem com os da faixa (e duração parecida).
fn pick(results: &[Value], title: &str, artist: &str, duration_ms: i64) -> Option<String> {
    let want_title = norm(&clean_title(title));
    // O player junta artistas ("A, B", "A e B"): basta o primeiro bater.
    let first_artist = artist.split([',', '&', ';']).next().map(norm).unwrap_or_default();
    let want_artist = norm(artist);
    if want_title.is_empty() || first_artist.is_empty() {
        return None;
    }
    results.iter().find_map(|r| {
        let t = norm(r.get("trackName")?.as_str()?);
        let a = norm(r.get("artistName")?.as_str()?);
        let title_ok = t == want_title || t.starts_with(&want_title) || want_title.starts_with(&t);
        let artist_ok = a.contains(&first_artist) || want_artist.contains(&a) || a == want_artist;
        let duration_ok = match r.get("trackTimeMillis").and_then(Value::as_i64) {
            Some(d) if duration_ms > 0 => (d - duration_ms).abs() <= MAX_DURATION_DIFF_MS,
            _ => true,
        };
        if !(title_ok && artist_ok && duration_ok) || t.is_empty() {
            return None;
        }
        let small = r.get("artworkUrl100")?.as_str()?;
        Some(upscale(small))
    })
}

/// ".../100x100bb.jpg" → ".../1000x1000bb.jpg"
fn upscale(url: &str) -> String {
    match url.rfind("100x100bb") {
        Some(i) => format!("{}{}{}", &url[..i], SIZE, &url[i + "100x100bb".len()..]),
        None => url.to_string(),
    }
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    let client = client().map_err(|e| e.to_string())?;
    let uri = Uri::CreateUri(&HSTRING::from(url)).map_err(|e| e.to_string())?;
    let buffer = client
        .GetBufferAsync(&uri)
        .and_then(|op| op.join())
        .map_err(|e| e.to_string())?;
    let reader = DataReader::FromBuffer(&buffer).map_err(|e| e.to_string())?;
    let mut out = vec![0u8; buffer.Length().map_err(|e| e.to_string())? as usize];
    reader.ReadBytes(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn upscales_itunes_url() {
        assert_eq!(
            upscale("https://is1-ssl.mzstatic.com/image/thumb/Music/x/100x100bb.jpg"),
            "https://is1-ssl.mzstatic.com/image/thumb/Music/x/1000x1000bb.jpg"
        );
    }

    #[test]
    fn picks_matching_track_only() {
        let results = vec![
            json!({"trackName": "Sunflower - Remix", "artistName": "Other", "trackTimeMillis": 155000, "artworkUrl100": "u/100x100bb.jpg"}),
            json!({"trackName": "Sunflower", "artistName": "Peach Luffe", "trackTimeMillis": 156000, "artworkUrl100": "good/100x100bb.jpg"}),
        ];
        assert_eq!(pick(&results, "Sunflower", "Peach Luffe", 155_000), Some("good/1000x1000bb.jpg".into()));
        // duração muito diferente = outra versão
        assert_eq!(pick(&results, "Sunflower", "Peach Luffe", 240_000), None);
        // artista diferente
        assert_eq!(pick(&results, "Sunflower", "Fulano", 155_000), None);
    }
}
