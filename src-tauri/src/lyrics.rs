//! Letras sincronizadas (LRC) do lrclib.net.
//!
//! Privacidade: a busca envia ao lrclib.net só título, artista, álbum e duração
//! da faixa que está tocando (e um User-Agent "Vitrolinha" com o endereço do projeto). Pode ser
//! desligada no menu da bandeja. HTTP pelo HttpClient do sistema (sem crate extra).
//!
//! Fluxo: faixa nova → thread busca (exato → busca por campos → busca por título
//! limpo) → evento `lyrics` com as linhas. O front sincroniza com a posição.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use windows::core::HSTRING;
use windows::Foundation::Uri;
use windows::Web::Http::HttpClient;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

use crate::media::{NowPlaying, SharedState};
use crate::settings;

const API: &str = "https://lrclib.net";
const USER_AGENT: &str = "Vitrolinha/0.1 (+https://github.com/guNaoki/vitrolinha)";
/// Com duração conhecida, só aceita letra de uma faixa com duração parecida
/// (evita casar com outra versão da música).
const MAX_DURATION_DIFF_S: f64 = 15.0;
const CACHE_LIMIT: usize = 40;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub time_ms: i64,
    pub text: String,
}

/// Contrato Rust → front (espelha `Lyrics` em src/lib/lyrics.ts).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    pub title: String,
    pub artist: String,
    pub lines: Vec<Line>,
}

#[derive(Default)]
pub struct LyricsState {
    current: Mutex<Option<Lyrics>>,
    /// Cache em memória (também guarda "sem letra", para não repetir a busca).
    cache: Mutex<HashMap<String, Option<Vec<Line>>>>,
    /// Cada pedido novo invalida os anteriores ainda em andamento.
    generation: AtomicU64,
}

/// Letra da faixa atual, se já chegou (o front chama ao abrir).
pub fn current(app: &AppHandle) -> Option<Lyrics> {
    app.state::<LyricsState>().current.lock().unwrap().clone()
}

/// Faixa nova (ou nenhuma): limpa a letra e, se estiver ligado, busca a nova.
pub fn request(app: &AppHandle, np: &NowPlaying) {
    let state = app.state::<LyricsState>();
    let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    *state.current.lock().unwrap() = None;
    let _ = app.emit("lyrics", None::<Lyrics>); // o front some com a letra antiga

    if np.title.is_empty() || !settings::lyrics_enabled(app) {
        return;
    }
    let key = format!(
        "{}|{}|{}|{}",
        np.artist.to_lowercase(),
        np.title.to_lowercase(),
        np.album.to_lowercase(),
        np.duration_ms / 1000
    );
    let cached = state.cache.lock().unwrap().get(&key).cloned();
    if let Some(lines) = cached {
        finish(app, generation, np, lines);
        return;
    }

    let (app, np) = (app.clone(), np.clone());
    thread::spawn(move || {
        let lines = match fetch(&np.title, &np.artist, &np.album, np.duration_ms / 1000) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[letras] falha na busca: {e}");
                return; // erro de rede: não guarda no cache, tenta de novo na próxima vez
            }
        };
        {
            let state = app.state::<LyricsState>();
            let mut cache = state.cache.lock().unwrap();
            if cache.len() >= CACHE_LIMIT {
                cache.clear();
            }
            cache.insert(key, lines.clone());
        }
        finish(&app, generation, &np, lines);
    });
}

/// Reaplica à faixa atual (ao ligar/desligar a opção).
pub fn reload(app: &AppHandle) {
    let current = app.state::<SharedState>().now.lock().unwrap().clone();
    match current {
        Some(np) => request(app, &np),
        None => {
            let _ = app.emit("lyrics", None::<Lyrics>);
        }
    }
}

fn finish(app: &AppHandle, generation: u64, np: &NowPlaying, lines: Option<Vec<Line>>) {
    let state = app.state::<LyricsState>();
    if state.generation.load(Ordering::SeqCst) != generation {
        return; // a faixa já mudou
    }
    let Some(lines) = lines else {
        println!("[letras] sem letra sincronizada: {} — {}", np.title, np.artist);
        return;
    };
    println!("[letras] {} linhas: {} — {}", lines.len(), np.title, np.artist);
    let lyrics = Lyrics { title: np.title.clone(), artist: np.artist.clone(), lines };
    *state.current.lock().unwrap() = Some(lyrics.clone());
    let _ = app.emit("lyrics", Some(lyrics));
}

// ---------------------------------------------------------------- busca

fn fetch(title: &str, artist: &str, album: &str, duration_s: i64) -> Result<Option<Vec<Line>>, String> {
    // Thread nova: garante o apartamento MTA do WinRT (S_FALSE se já estiver).
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
    let client = http_client().map_err(|e| e.to_string())?;
    let dur = duration_s.to_string();
    let clean = clean_title(title);
    let mut network_ok = false;

    // 1) casamento exato (precisa de álbum e duração)
    if !album.is_empty() && duration_s > 0 {
        if let Some(v) = get_json(&client, "/api/get", &[("track_name", title), ("artist_name", artist), ("album_name", album), ("duration", &dur)]) {
            network_ok = true;
            if let Some(lines) = synced_lines(&v) {
                return Ok(Some(lines));
            }
        }
    }
    // 2) busca por título + artista, 3) busca livre pelo título limpo
    let searches: [Vec<(&str, &str)>; 2] = [
        vec![("track_name", clean.as_str()), ("artist_name", artist)],
        vec![("q", clean.as_str())],
    ];
    for params in searches {
        if clean.is_empty() {
            break;
        }
        if let Some(Value::Array(results)) = get_json(&client, "/api/search", &params) {
            network_ok = true;
            if let Some(lines) = pick_best(&results, duration_s) {
                return Ok(Some(lines));
            }
        }
    }
    if network_ok { Ok(None) } else { Err("lrclib.net inacessível".into()) }
}

fn http_client() -> windows::core::Result<HttpClient> {
    let client = HttpClient::new()?;
    client.DefaultRequestHeaders()?.UserAgent()?.TryParseAdd(&HSTRING::from(USER_AGENT))?;
    Ok(client)
}

/// GET → JSON. `None` em qualquer falha (404 inclusive).
fn get_json(client: &HttpClient, path: &str, params: &[(&str, &str)]) -> Option<Value> {
    let mut url = format!("{API}{path}?");
    for (i, (k, v)) in params.iter().enumerate() {
        if i > 0 {
            url.push('&');
        }
        let encoded = Uri::EscapeComponent(&HSTRING::from(*v)).ok()?.to_string();
        url.push_str(&format!("{k}={encoded}"));
    }
    let uri = Uri::CreateUri(&HSTRING::from(url)).ok()?;
    let body = client.GetStringAsync(&uri).ok()?.join().ok()?.to_string();
    serde_json::from_str(&body).ok()
}

fn synced_lines(v: &Value) -> Option<Vec<Line>> {
    let lrc = v.get("syncedLyrics")?.as_str()?;
    let lines = parse_lrc(lrc);
    (!lines.is_empty()).then_some(lines)
}

/// Entre os resultados com letra sincronizada, o de duração mais próxima
/// (e dentro do limite, se a duração da faixa é conhecida).
fn pick_best(results: &[Value], duration_s: i64) -> Option<Vec<Line>> {
    results
        .iter()
        .filter_map(|v| {
            let lines = synced_lines(v)?;
            let diff = match (duration_s, v.get("duration").and_then(Value::as_f64)) {
                (d, Some(rd)) if d > 0 => (rd - d as f64).abs(),
                _ => 0.0,
            };
            (diff <= MAX_DURATION_DIFF_S).then_some((diff, lines))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, lines)| lines)
}

// ---------------------------------------------------------------- texto

/// Tira trechos entre () e [] ("(cover)", "[Official Video]") e espaços sobrando.
pub fn clean_title(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0u32;
    for c in s.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// LRC → linhas ordenadas por tempo. Aceita várias marcas por linha
/// (`[00:10.00][01:30.00]refrão`) e ignora tags de metadados (`[ar:...]`).
pub fn parse_lrc(text: &str) -> Vec<Line> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some(end) = stripped.find(']') else { break };
            let Some(t) = parse_timestamp(&stripped[..end]) else { break };
            times.push(t);
            rest = stripped[end + 1..].trim_start();
        }
        for time_ms in times {
            out.push(Line { time_ms, text: rest.trim().to_string() });
        }
    }
    out.sort_by_key(|l| l.time_ms);
    out
}

/// "mm:ss", "mm:ss.xx" ou "mm:ss.xxx" → ms.
fn parse_timestamp(tag: &str) -> Option<i64> {
    let (min, rest) = tag.split_once(':')?;
    let (sec, frac) = rest.split_once('.').unwrap_or((rest, ""));
    let (min, sec): (i64, i64) = (min.trim().parse().ok()?, sec.trim().parse().ok()?);
    if !frac.chars().all(|c| c.is_ascii_digit()) || frac.len() > 3 {
        return None;
    }
    let ms: i64 = if frac.is_empty() { 0 } else { format!("{frac:0<3}").parse().ok()? };
    Some((min * 60 + sec) * 1000 + ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_timestamps_and_text() {
        let lines = parse_lrc("[00:12.50]Olá\n[01:05.07]Mundo\n[02:00]Fim");
        assert_eq!(
            lines,
            vec![
                Line { time_ms: 12_500, text: "Olá".into() },
                Line { time_ms: 65_070, text: "Mundo".into() },
                Line { time_ms: 120_000, text: "Fim".into() },
            ]
        );
    }

    #[test]
    fn ignores_metadata_tags_and_garbage() {
        let lines = parse_lrc("[ar:Alguém]\n[ti:Algo]\n[length: 03:20]\nsem marca\n[00:01.00]ok");
        assert_eq!(lines, vec![Line { time_ms: 1_000, text: "ok".into() }]);
    }

    #[test]
    fn handles_multiple_stamps_and_sorts() {
        let lines = parse_lrc("[01:00.00][00:10.00]refrão\n[00:30.00]verso");
        let times: Vec<_> = lines.iter().map(|l| l.time_ms).collect();
        assert_eq!(times, vec![10_000, 30_000, 60_000]);
        assert_eq!(lines[0].text, "refrão");
        assert_eq!(lines[2].text, "refrão");
    }

    #[test]
    fn keeps_empty_lines_as_gaps() {
        let lines = parse_lrc("[00:05.00]\n[00:09.00]canta");
        assert_eq!(lines[0].text, "");
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn fraction_digits_are_padded() {
        assert_eq!(parse_timestamp("00:01.5"), Some(1_500));
        assert_eq!(parse_timestamp("00:01.05"), Some(1_050));
        assert_eq!(parse_timestamp("00:01.005"), Some(1_005));
        assert_eq!(parse_timestamp("00:01.1234"), None);
        assert_eq!(parse_timestamp("ar:Alguém"), None);
    }

    #[test]
    fn cleans_noisy_titles() {
        assert_eq!(clean_title("Always - Daniel Caesar (cover)"), "Always - Daniel Caesar");
        assert_eq!(clean_title("Song [Official Video] (Live)  "), "Song");
        assert_eq!(clean_title("Plain"), "Plain");
    }

    #[test]
    fn picks_closest_duration_within_limit() {
        let r: Vec<Value> = serde_json::from_str(
            r#"[
              {"duration": 300, "syncedLyrics": "[00:01.00]longe"},
              {"duration": 201, "syncedLyrics": "[00:01.00]perto"},
              {"duration": 200, "syncedLyrics": null}
            ]"#,
        )
        .unwrap();
        assert_eq!(pick_best(&r, 200).unwrap()[0].text, "perto");
        assert!(pick_best(&r[..1], 200).is_none(), "100s de diferença não serve");
        assert_eq!(pick_best(&r[..1], 0).unwrap()[0].text, "longe", "sem duração conhecida, aceita");
    }
}
