//! "Ouvindo agora" no perfil do Discord (opcional, desligado por padrão).
//!
//! Usa uma conexão própria com o Discord instalado (IPC local, `\\.\pipe\discord-ipc-N`), separada
//! da lista da call: assim dá para escrever a qualquer momento sem brigar com a leitura da outra.
//! Não precisa de autorização (OAuth), só do Client ID do app (o mesmo da call, em discord.json).
//! Só sai do computador o que o Discord mostra no perfil: título e artista da faixa que toca.
//!
//! Uma thread parada em `recv` (sem polling). Cada mudança de faixa/estado do `media` a acorda;
//! ela lê o estado atual e atualiza (ou limpa) a atividade. Sem Discord aberto, não faz nada.

use std::fs::File;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::discord::{connect, read_frame, write_frame, OP_FRAME, OP_HANDSHAKE, OP_PING, OP_PONG};
use crate::media::{now_ms, SharedState};
use crate::settings;

/// O Discord aceita poucas atualizações por vez (5 a cada 20 s): junta as rajadas (pular várias
/// faixas seguidas) e só envia a última.
const MIN_GAP: Duration = Duration::from_secs(4);

#[derive(Default)]
pub struct PresenceState {
    tx: Mutex<Option<Sender<()>>>,
}

/// Avisa que algo mudou (faixa, tocar/pausar, posição, ou a opção no menu).
pub fn update(app: &AppHandle) {
    let state = app.state::<PresenceState>();
    let mut tx = state.tx.lock().unwrap();
    if tx.is_none() {
        // A thread só nasce na primeira mudança, e só se a opção está ligada: com ela desligada
        // (o padrão) nada é criado.
        if !settings::discord_presence(app) {
            return;
        }
        let (sender, rx) = channel();
        let app = app.clone();
        thread::Builder::new()
            .name("discord-presence".into())
            .spawn(move || run(app, rx))
            .expect("não foi possível criar a thread da presença");
        *tx = Some(sender);
    }
    let _ = tx.as_ref().unwrap().send(());
}

fn run(app: AppHandle, rx: Receiver<()>) {
    let mut pipe: Option<File> = None;
    let mut last_sent: Option<Instant> = None;
    while rx.recv().is_ok() {
        // Espera o intervalo mínimo juntando o que chegar nesse meio-tempo.
        if let Some(t) = last_sent {
            let deadline = t + MIN_GAP;
            loop {
                match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(()) => continue,
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        }
        while rx.try_recv().is_ok() {}

        match activity(&app) {
            Some((client_id, activity)) => match send(&mut pipe, &client_id, &activity) {
                Ok(()) => last_sent = Some(Instant::now()),
                Err(e) => {
                    eprintln!("[presence] falha: {e}");
                    pipe = None; // tenta reconectar na próxima mudança
                }
            },
            // Desligado, pausado ou sem faixa: fechar a conexão faz o Discord limpar a atividade.
            None => {
                if pipe.take().is_some() {
                    eprintln!("[presence] atividade removida");
                }
            }
        }
    }
}

/// (client id, atividade) se é para mostrar agora; `None` para limpar.
fn activity(app: &AppHandle) -> Option<(String, Value)> {
    if !settings::discord_presence(app) {
        return None;
    }
    let (client_id, _) = settings::discord_credentials(app)?;
    let np = app.state::<SharedState>().now.lock().unwrap().clone()?;
    if np.status != "playing" || np.title.is_empty() {
        return None;
    }
    let now = now_ms();
    let start = now - np.position_at(now);
    let mut timestamps = json!({ "start": start / 1000 });
    if np.duration_ms > 0 {
        timestamps["end"] = json!((start + np.duration_ms) / 1000);
    }
    let mut a = json!({
        "type": 2, // "Ouvindo"
        "details": field(&np.title),
        "timestamps": timestamps,
    });
    if !np.artist.is_empty() {
        a["state"] = json!(field(&np.artist));
    }
    Some((client_id, a))
}

/// O Discord exige de 2 a 128 caracteres.
fn field(s: &str) -> String {
    let mut out: String = s.chars().take(128).collect();
    if out.chars().count() < 2 {
        out.push('\u{a0}');
    }
    out
}

fn send(pipe: &mut Option<File>, client_id: &str, activity: &Value) -> Result<(), String> {
    if pipe.is_none() {
        let mut p = connect().ok_or("Discord não está aberto")?;
        write_frame(&mut p, OP_HANDSHAKE, &json!({ "v": 1, "client_id": client_id }))?;
        let (_, ready) = read_frame(&mut p)?; // DISPATCH/READY
        if ready["evt"] != "READY" {
            return Err(format!("handshake recusado: {}", ready["data"]["message"]));
        }
        *pipe = Some(p);
    }
    let p = pipe.as_mut().unwrap();
    let nonce = format!("presence-{}", now_ms());
    write_frame(
        p,
        OP_FRAME,
        &json!({
            "cmd": "SET_ACTIVITY",
            "args": { "pid": std::process::id(), "activity": activity },
            "nonce": nonce,
        }),
    )?;
    // A resposta confirma ou diz o motivo do erro (só código e mensagem entram no log).
    loop {
        let (op, msg) = read_frame(p)?;
        if op == OP_PING {
            write_frame(p, OP_PONG, &msg)?;
            continue;
        }
        if msg["nonce"] == nonce.as_str() {
            if msg["evt"] == "ERROR" {
                return Err(format!("o Discord recusou: {} {}", msg["data"]["code"], msg["data"]["message"]));
            }
            eprintln!("[presence] atualizada (tipo {})", msg["data"]["type"]);
            return Ok(());
        }
    }
}
