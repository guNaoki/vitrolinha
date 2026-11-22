//! Overlay para OBS e streamers (opcional, desligado por padrão): uma página mínima servida em
//! `http://localhost:7421` que mostra a faixa atual com a cara da Vitrolinha. No OBS: fonte
//! "Navegador" com esse endereço (sugestão: 520 × 170, fundo transparente).
//!
//! Servidor HTTP mínimo só em 127.0.0.1 (nada acessível pela rede), uma thread parada em
//! `accept`. Só responde a `Host: localhost|127.0.0.1:porta`, o que impede que um site aberto no
//! navegador, por DNS rebinding, leia o que você ouve. Quem usa o OBS vê apenas título, artista,
//! capa e posição (o mesmo que a página mostra).

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::media::{self, SharedState};
use crate::settings;

pub const PORT: u16 = 7421;

const PAGE: &str = include_str!("../overlay/overlay.html");
const FONT_KRONA: &[u8] = include_bytes!("../../src/assets/fonts/krona-one-latin-400-normal.woff2");
const FONT_FELL: &[u8] = include_bytes!("../../src/assets/fonts/im-fell-english-latin-400-normal.woff2");
const FONT_FELL_ITALIC: &[u8] = include_bytes!("../../src/assets/fonts/im-fell-english-latin-400-italic.woff2");

#[derive(Default)]
pub struct OverlayState {
    /// Cada `reload` invalida o servidor anterior (ele encerra no próximo pedido).
    generation: AtomicU64,
    /// Há um servidor no ar (só então vale a pena acordá-lo ao recarregar).
    serving: AtomicBool,
}

/// (Re)liga ou desliga conforme a configuração: na inicialização e ao mexer no menu.
pub fn reload(app: &AppHandle) {
    let state = app.state::<OverlayState>();
    let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    if state.serving.load(Ordering::SeqCst) {
        wake(); // acorda o servidor antigo (parado em `accept`) para ele perceber e sair
    }
    if !settings::overlay_enabled(app) {
        return;
    }
    let app = app.clone();
    let _ = thread::Builder::new().name("overlay-http".into()).spawn(move || serve(app, generation));
}

fn addr() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, PORT))
}

/// Conexão vazia só para desbloquear o `accept` do servidor anterior.
fn wake() {
    let _ = TcpStream::connect_timeout(&addr(), Duration::from_millis(200));
}

fn serve(app: AppHandle, generation: u64) {
    // O servidor antigo pode ainda estar saindo: tenta por um instante.
    let listener = (0..10).find_map(|_| match TcpListener::bind(addr()) {
        Ok(l) => Some(l),
        Err(_) => {
            thread::sleep(Duration::from_millis(150));
            None
        }
    });
    let Some(listener) = listener else {
        eprintln!("[overlay] a porta {PORT} está ocupada; overlay indisponível");
        return;
    };
    eprintln!("[overlay] ligado em http://localhost:{PORT}");
    app.state::<OverlayState>().serving.store(true, Ordering::SeqCst);
    for conn in listener.incoming() {
        let state = app.state::<OverlayState>();
        if state.generation.load(Ordering::SeqCst) != generation {
            eprintln!("[overlay] encerrado");
            // Se não há servidor mais novo, o overlay está desligado.
            if !settings::overlay_enabled(&app) {
                state.serving.store(false, Ordering::SeqCst);
            }
            return;
        }
        let Ok(stream) = conn else { continue };
        let app = app.clone();
        let _ = thread::Builder::new().name("overlay-conn".into()).spawn(move || {
            let _ = handle(&app, stream);
        });
    }
}

fn respond(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8], cache: &str) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: {cache}\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}

fn handle(app: &AppHandle, mut stream: TcpStream) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    // Só o cabeçalho interessa (pedidos GET): até 8 KB.
    let mut buf = [0u8; 8192];
    let mut len = 0;
    while len < buf.len() {
        let n = stream.read(&mut buf[len..])?;
        if n == 0 {
            break;
        }
        len += n;
        if buf[..len].windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let head = String::from_utf8_lossy(&buf[..len]).into_owned();
    let mut lines = head.split("\r\n");
    let request = lines.next().unwrap_or("");
    let host = lines
        .find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case("host")).map(|(_, v)| v.trim().to_ascii_lowercase()))
        .unwrap_or_default();
    // Defesa contra DNS rebinding: só aceita os nomes locais com a nossa porta.
    if host != format!("localhost:{PORT}") && host != format!("127.0.0.1:{PORT}") {
        return respond(&mut stream, "403 Forbidden", "text/plain", b"forbidden", "no-store");
    }
    let mut parts = request.split(' ');
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));
    if method != "GET" {
        return respond(&mut stream, "405 Method Not Allowed", "text/plain", b"method not allowed", "no-store");
    }
    let path = target.split('?').next().unwrap_or("/");
    match path {
        "/" => respond(&mut stream, "200 OK", "text/html; charset=utf-8", PAGE.as_bytes(), "no-store"),
        "/state" => respond(&mut stream, "200 OK", "application/json", state_json(app).as_bytes(), "no-store"),
        "/cover" => match cover_bytes(app) {
            Some((mime, bytes)) => respond(&mut stream, "200 OK", mime, &bytes, "no-store"),
            None => respond(&mut stream, "404 Not Found", "text/plain", b"no cover", "no-store"),
        },
        "/fonts/krona-one-latin-400-normal.woff2" => respond(&mut stream, "200 OK", "font/woff2", FONT_KRONA, "max-age=86400"),
        "/fonts/im-fell-english-latin-400-normal.woff2" => respond(&mut stream, "200 OK", "font/woff2", FONT_FELL, "max-age=86400"),
        "/fonts/im-fell-english-latin-400-italic.woff2" => respond(&mut stream, "200 OK", "font/woff2", FONT_FELL_ITALIC, "max-age=86400"),
        _ => respond(&mut stream, "404 Not Found", "text/plain", b"not found", "no-store"),
    }
}

/// Estado atual para a página (sem caminhos de arquivo do computador).
fn state_json(app: &AppHandle) -> String {
    let np = app.state::<SharedState>().now.lock().unwrap().clone().map(|n| media::for_ui(app, &n));
    match np {
        Some(n) => json!({
            "title": n.title,
            "artist": n.artist,
            "status": n.status,
            "positionMs": n.position_ms,
            "durationMs": n.duration_ms,
            "sampledAtMs": n.sampled_at_ms,
            "accentColor": n.accent_color,
            "hasCover": n.artwork_path.is_some(),
            // Muda quando a capa muda (a página só recarrega a imagem nesse caso).
            "coverKey": n.artwork_path.as_deref().and_then(|p| std::path::Path::new(p).file_name()).map(|f| f.to_string_lossy().into_owned()),
        })
        .to_string(),
        None => json!({ "status": "stopped" }).to_string(),
    }
}

/// A capa da faixa atual (JPEG ou PNG), lida do arquivo temporário do app.
fn cover_bytes(app: &AppHandle) -> Option<(&'static str, Vec<u8>)> {
    let np = app.state::<SharedState>().now.lock().unwrap().clone().map(|n| media::for_ui(app, &n))?;
    let path = std::path::Path::new(np.artwork_path.as_deref()?).canonicalize().ok()?;
    if !crate::share::cover_dirs(app).iter().any(|d| path.starts_with(d)) {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    let mime = if bytes.starts_with(b"\x89PNG") { "image/png" } else { "image/jpeg" };
    Some((mime, bytes))
}
