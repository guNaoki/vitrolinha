//! Call de voz do Discord no painel (tela cheia): quem está no canal, quem está falando,
//! quem está mudo/surdo.
//!
//! Fala com o app do Discord instalado neste computador pelo IPC local (named pipe
//! `\\.\pipe\discord-ipc-N`), o mesmo canal que os overlays usam. Só leitura.
//!
//! Precisa de um aplicativo próprio no Discord Developer Portal (client id e secret no
//! settings.json, ver README). Na 1ª vez o Discord pede para autorizar o app; o token
//! fica guardado no settings.json e é renovado sozinho.
//!
//! Privacidade: a conversa com o Discord é toda local. A única chamada de rede é a troca
//! do código de autorização por token (discord.com/api/oauth2/token), feita uma vez a
//! cada ~7 dias. Os avatares o painel carrega direto do CDN do Discord.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use windows::core::HSTRING;
use windows::Foundation::Uri;
use windows::Storage::Streams::UnicodeEncoding;
use windows::Web::Http::{HttpClient, HttpStringContent};
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

use crate::settings::{self, DiscordToken};

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;
const OP_PING: u32 = 3;
const OP_PONG: u32 = 4;
/// Teto de um quadro (um canal grande cabe folgado).
const MAX_FRAME: usize = 8 * 1024 * 1024;

const SCOPES: [&str; 2] = ["rpc", "rpc.voice.read"];
/// Eventos por canal de voz, assinados ao entrar e cancelados ao sair.
const CHANNEL_EVENTS: [&str; 5] = [
    "VOICE_STATE_CREATE",
    "VOICE_STATE_UPDATE",
    "VOICE_STATE_DELETE",
    "SPEAKING_START",
    "SPEAKING_STOP",
];
const TOKEN_URL: &str = "https://discord.com/api/oauth2/token";
const USER_AGENT: &str = "Flyout/0.1 (desktop music flyout)";
const RETRY_EVERY: Duration = Duration::from_secs(5);

// ---------------------------------------------------------------- contrato com o front

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Opção desligada.
    #[default]
    Off,
    /// Ligada, mas faltam client id/secret (ou o Discord não os aceitou).
    Setup,
    /// Ligada; o Discord não está aberto (tentando de novo a cada poucos segundos).
    Waiting,
    /// O Discord está pedindo para autorizar o app.
    Authorizing,
    /// A autorização foi recusada ou falhou; religar a opção no menu tenta de novo.
    Refused,
    Connected,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub id: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub muted: bool,
    pub deafened: bool,
    pub speaking: bool,
}

/// Espelha `Voice` em src/lib/voice.ts. Evento `voice`.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    pub status: Status,
    /// Nome do canal de voz (ou da chamada); `None` = fora de qualquer call.
    pub channel: Option<String>,
    pub members: Vec<Member>,
}

#[derive(Default)]
pub struct DiscordState {
    voice: Mutex<Voice>,
    /// Cada `reload` invalida a conexão anterior (ela se encerra no próximo quadro).
    generation: AtomicU64,
}

pub fn current(app: &AppHandle) -> Voice {
    app.state::<DiscordState>().voice.lock().unwrap().clone()
}

fn publish(app: &AppHandle, voice: Voice) {
    let state = app.state::<DiscordState>();
    {
        let mut cur = state.voice.lock().unwrap();
        if *cur == voice {
            return;
        }
        *cur = voice.clone();
    }
    let _ = app.emit("voice", voice);
}

fn only_status(app: &AppHandle, status: Status) {
    publish(app, Voice { status, ..Voice::default() });
}

/// (Re)começa conforme a configuração: chamada na inicialização e ao ligar/desligar no menu.
pub fn reload(app: &AppHandle) {
    let generation = app.state::<DiscordState>().generation.fetch_add(1, Ordering::SeqCst) + 1;
    only_status(app, Status::Off);
    eprintln!(
        "[discord] reload: ligado={} credenciais={}",
        settings::discord_enabled(app),
        settings::discord_credentials(app).is_some()
    );
    if !settings::discord_enabled(app) {
        return;
    }
    let app = app.clone();
    thread::spawn(move || supervise(app, generation));
}

// ---------------------------------------------------------------- supervisor

/// Por que uma conexão acabou.
enum End {
    /// Tenta de novo daqui a pouco (Discord fechou/reiniciou).
    Retry(String),
    /// Não adianta insistir: mostra o status e para até o usuário religar a opção.
    Stop(Status, String),
}

impl From<String> for End {
    fn from(e: String) -> Self {
        End::Retry(e)
    }
}

fn supervise(app: AppHandle, generation: u64) {
    // Thread nova: garante o apartamento MTA do WinRT (HttpClient da troca de token).
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
    let stale = || app.state::<DiscordState>().generation.load(Ordering::SeqCst) != generation;

    let Some((client_id, secret)) = settings::discord_credentials(&app) else {
        eprintln!("[discord] faltam discordClientId/discordClientSecret no settings.json");
        only_status(&app, Status::Setup);
        return;
    };
    eprintln!("[discord] iniciado (geração {generation})");
    while !stale() {
        let pipe = connect();
        eprintln!("[discord] pipe: {}", if pipe.is_some() { "aberto" } else { "não encontrado" });
        if let Some(pipe) = pipe {
            let mut session = Session::new(&app, generation, pipe, client_id.clone(), secret.clone());
            match session.run() {
                Ok(()) => return, // `reload` já assumiu
                Err(End::Stop(status, why)) => {
                    eprintln!("[discord] parou: {why}");
                    if !stale() {
                        only_status(&app, status);
                    }
                    return;
                }
                Err(End::Retry(why)) => eprintln!("[discord] conexão encerrada: {why}"),
            }
        }
        if stale() {
            return;
        }
        only_status(&app, Status::Waiting);
        thread::sleep(RETRY_EVERY);
    }
}

/// Abre o primeiro pipe do Discord que responder (discord-ipc-0…9: 0 é o Discord comum,
/// os seguintes valem quando há mais de uma instância, como Canary/PTB).
fn connect() -> Option<File> {
    (0..10).find_map(|i| {
        OpenOptions::new().read(true).write(true).open(format!(r"\\.\pipe\discord-ipc-{i}")).ok()
    })
}

// ---------------------------------------------------------------- quadros

fn write_frame(pipe: &mut File, op: u32, payload: &Value) -> Result<(), String> {
    let body = serde_json::to_vec(payload).map_err(|e| e.to_string())?;
    let mut buf = Vec::with_capacity(8 + body.len());
    buf.extend_from_slice(&op.to_le_bytes());
    buf.extend_from_slice(&(body.len() as u32).to_le_bytes());
    buf.extend_from_slice(&body);
    pipe.write_all(&buf).map_err(|e| format!("escrita no pipe: {e}"))
}

fn read_frame(pipe: &mut File) -> Result<(u32, Value), String> {
    let mut head = [0u8; 8];
    pipe.read_exact(&mut head).map_err(|e| format!("leitura do pipe: {e}"))?;
    let op = u32::from_le_bytes(head[0..4].try_into().unwrap());
    let len = u32::from_le_bytes(head[4..8].try_into().unwrap()) as usize;
    if len > MAX_FRAME {
        return Err(format!("quadro grande demais ({len} bytes)"));
    }
    let mut body = vec![0u8; len];
    pipe.read_exact(&mut body).map_err(|e| format!("leitura do pipe: {e}"))?;
    let value = serde_json::from_slice(&body).map_err(|e| format!("JSON inválido: {e}"))?;
    Ok((op, value))
}

// ---------------------------------------------------------------- sessão

struct Session<'a> {
    app: &'a AppHandle,
    generation: u64,
    pipe: File,
    client_id: String,
    secret: String,
    /// O token que acabamos de obter (não o guardado) foi recusado: não há mais o que tentar.
    fresh_token: bool,
    voice: Voice,
    /// Canal cujos eventos estão assinados.
    subscribed: Option<String>,
}

impl<'a> Session<'a> {
    fn new(app: &'a AppHandle, generation: u64, pipe: File, client_id: String, secret: String) -> Self {
        Session {
            app,
            generation,
            pipe,
            client_id,
            secret,
            fresh_token: false,
            voice: Voice { status: Status::Waiting, ..Voice::default() },
            subscribed: None,
        }
    }

    fn stale(&self) -> bool {
        self.app.state::<DiscordState>().generation.load(Ordering::SeqCst) != self.generation
    }

    /// Só publica se esta conexão ainda é a vigente.
    fn publish(&self) {
        if !self.stale() {
            publish(self.app, self.voice.clone());
        }
    }

    fn run(&mut self) -> Result<(), End> {
        write_frame(&mut self.pipe, OP_HANDSHAKE, &json!({ "v": 1, "client_id": self.client_id }))?;
        eprintln!("[discord] handshake enviado");
        loop {
            let (op, msg) = read_frame(&mut self.pipe)?;
            // Só cmd/evt/nonce: o resto pode ter token ou dados de outras pessoas.
            eprintln!("[discord] recebido op={op} cmd={} evt={} nonce={}", msg["cmd"], msg["evt"], msg["nonce"]);
            if self.stale() {
                return Ok(());
            }
            match op {
                OP_PING => write_frame(&mut self.pipe, OP_PONG, &msg)?,
                OP_CLOSE => {
                    let code = msg["code"].as_i64().unwrap_or(0);
                    let why = format!("Discord fechou a conexão ({code}): {}", msg["message"].as_str().unwrap_or(""));
                    // 4000 = client id inválido: tentar de novo não resolve.
                    return Err(if code == 4000 { End::Stop(Status::Setup, why) } else { End::Retry(why) });
                }
                OP_FRAME => self.on_frame(&msg)?,
                _ => {}
            }
        }
    }

    fn send(&mut self, cmd: &str, args: Value, nonce: &str) -> Result<(), String> {
        write_frame(&mut self.pipe, OP_FRAME, &json!({ "cmd": cmd, "args": args, "nonce": nonce }))
    }

    fn subscribe(&mut self, evt: &str, args: Value, on: bool) -> Result<(), String> {
        let cmd = if on { "SUBSCRIBE" } else { "UNSUBSCRIBE" };
        write_frame(&mut self.pipe, OP_FRAME, &json!({ "cmd": cmd, "evt": evt, "args": args, "nonce": format!("{cmd}:{evt}") }))
    }

    fn on_frame(&mut self, m: &Value) -> Result<(), End> {
        let evt = m["evt"].as_str().unwrap_or("");
        let nonce = m["nonce"].as_str().unwrap_or("");
        if evt == "ERROR" {
            return self.on_error(nonce, &m["data"]);
        }
        if m["cmd"] == "DISPATCH" {
            return self.on_dispatch(evt, &m["data"]);
        }
        match nonce {
            "authz" => self.on_authorized(&m["data"]),
            "authn" => self.on_authenticated(),
            "sel" => {
                self.on_selected(&m["data"])?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    // ---- autorização

    /// Pronto para conversar: usa o token guardado ou pede autorização.
    fn login(&mut self) -> Result<(), End> {
        match settings::discord_token(self.app) {
            Some(token) => Ok(self.send("AUTHENTICATE", json!({ "access_token": token.access }), "authn")?),
            None => self.authorize(),
        }
    }

    fn authorize(&mut self) -> Result<(), End> {
        eprintln!("[discord] pedindo autorização ao Discord");
        self.voice.status = Status::Authorizing;
        self.publish();
        Ok(self.send("AUTHORIZE", json!({ "client_id": self.client_id, "scopes": SCOPES }), "authz")?)
    }

    /// O usuário aprovou no Discord: troca o código por token e autentica.
    fn on_authorized(&mut self, data: &Value) -> Result<(), End> {
        let code = data["code"].as_str().ok_or_else(|| End::Stop(Status::Refused, "AUTHORIZE sem código".into()))?;
        let redirect = settings::discord_redirect_uri(self.app);
        let token = token_request(&[
            ("client_id", &self.client_id),
            ("client_secret", &self.secret),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &redirect),
        ])
        .map_err(|e| End::Stop(Status::Refused, format!("troca de código por token: {e}")))?;
        settings::set_discord_token(self.app, Some(&token));
        self.fresh_token = true;
        Ok(self.send("AUTHENTICATE", json!({ "access_token": token.access }), "authn")?)
    }

    fn on_authenticated(&mut self) -> Result<(), End> {
        self.voice.status = Status::Connected;
        self.subscribe("VOICE_CHANNEL_SELECT", json!({}), true)?;
        self.send("GET_SELECTED_VOICE_CHANNEL", json!({}), "sel")?;
        self.publish();
        Ok(())
    }

    fn on_error(&mut self, nonce: &str, data: &Value) -> Result<(), End> {
        let why = format!("{nonce}: {} ({})", data["message"].as_str().unwrap_or("?"), data["code"]);
        match nonce {
            // Token guardado vencido/revogado: tenta renovar; sem renovação, autoriza de novo.
            "authn" if !self.fresh_token => {
                eprintln!("[discord] token recusado ({why}); renovando");
                let refresh = settings::discord_token(self.app).and_then(|t| t.refresh);
                if let Some(refresh) = refresh {
                    let renewed = token_request(&[
                        ("client_id", &self.client_id),
                        ("client_secret", &self.secret),
                        ("grant_type", "refresh_token"),
                        ("refresh_token", &refresh),
                    ]);
                    if let Ok(token) = renewed {
                        settings::set_discord_token(self.app, Some(&token));
                        self.fresh_token = true;
                        return Ok(self.send("AUTHENTICATE", json!({ "access_token": token.access }), "authn")?);
                    }
                }
                settings::set_discord_token(self.app, None);
                self.authorize()
            }
            "authn" => Err(End::Stop(Status::Refused, format!("autenticação recusada: {why}"))),
            // Usuário cancelou o pedido de autorização no Discord.
            "authz" => Err(End::Stop(Status::Refused, format!("autorização recusada: {why}"))),
            // Outros comandos podem falhar sem derrubar a lista (ex.: canal que sumiu).
            _ => {
                eprintln!("[discord] erro do Discord: {why}");
                Ok(())
            }
        }
    }

    // ---- canal de voz

    fn on_dispatch(&mut self, evt: &str, data: &Value) -> Result<(), End> {
        match evt {
            "READY" => return self.login(),
            // Entrou, saiu ou trocou de canal: relê o canal atual.
            "VOICE_CHANNEL_SELECT" => self.send("GET_SELECTED_VOICE_CHANNEL", json!({}), "sel")?,
            "VOICE_STATE_CREATE" | "VOICE_STATE_UPDATE" => {
                if let Some(member) = parse_member(data) {
                    match self.voice.members.iter_mut().find(|m| m.id == member.id) {
                        Some(existing) => *existing = Member { speaking: existing.speaking, ..member },
                        None => self.voice.members.push(member),
                    }
                }
            }
            "VOICE_STATE_DELETE" => {
                if let Some(id) = data["user"]["id"].as_str() {
                    self.voice.members.retain(|m| m.id != id);
                }
            }
            "SPEAKING_START" | "SPEAKING_STOP" => {
                if let Some(id) = data["user_id"].as_str() {
                    if let Some(m) = self.voice.members.iter_mut().find(|m| m.id == id) {
                        m.speaking = evt == "SPEAKING_START";
                    }
                }
            }
            _ => {}
        }
        self.publish();
        Ok(())
    }

    /// Resposta de GET_SELECTED_VOICE_CHANNEL: `null` (fora de call) ou o canal com os estados de voz.
    fn on_selected(&mut self, data: &Value) -> Result<(), String> {
        let Some(id) = data["id"].as_str() else {
            self.watch(None)?;
            self.voice.channel = None;
            self.voice.members.clear();
            self.publish();
            return Ok(());
        };
        self.voice.channel = Some(data["name"].as_str().filter(|n| !n.is_empty()).unwrap_or("Chamada").to_string());
        self.voice.members = data["voice_states"].as_array().map(|a| a.iter().filter_map(parse_member).collect()).unwrap_or_default();
        self.watch(Some(id))?;
        self.publish();
        Ok(())
    }

    /// Assina os eventos do canal `id` (cancelando os do anterior); `None` só cancela.
    fn watch(&mut self, id: Option<&str>) -> Result<(), String> {
        if self.subscribed.as_deref() == id {
            return Ok(());
        }
        if let Some(old) = self.subscribed.take() {
            for evt in CHANNEL_EVENTS {
                self.subscribe(evt, json!({ "channel_id": old }), false)?;
            }
        }
        if let Some(id) = id {
            for evt in CHANNEL_EVENTS {
                self.subscribe(evt, json!({ "channel_id": id }), true)?;
            }
            self.subscribed = Some(id.to_string());
        }
        Ok(())
    }
}

/// Um estado de voz (da lista inicial ou de VOICE_STATE_*) → membro da lista.
fn parse_member(v: &Value) -> Option<Member> {
    let user = v.get("user")?;
    let id = user.get("id")?.as_str()?.to_string();
    let name = [v["nick"].as_str(), user["global_name"].as_str(), user["username"].as_str()]
        .into_iter()
        .flatten()
        .find(|s| !s.is_empty())
        .unwrap_or("?")
        .to_string();
    let state = &v["voice_state"];
    let flag = |k: &str| state[k].as_bool().unwrap_or(false);
    let avatar_url = user["avatar"]
        .as_str()
        .filter(|h| !h.is_empty())
        .map(|hash| format!("https://cdn.discordapp.com/avatars/{id}/{hash}.png?size=64"));
    Some(Member {
        id,
        name,
        avatar_url,
        muted: flag("self_mute") || flag("mute"),
        deafened: flag("self_deaf") || flag("deaf"),
        speaking: false,
    })
}

// ---------------------------------------------------------------- token (HTTP)

/// POST de formulário em /oauth2/token (código → token, ou renovação).
fn token_request(form: &[(&str, &str)]) -> Result<DiscordToken, String> {
    let e = |e: windows::core::Error| e.to_string();
    let body = form
        .iter()
        .map(|(k, v)| Ok(format!("{k}={}", Uri::EscapeComponent(&HSTRING::from(*v)).map_err(e)?)))
        .collect::<Result<Vec<_>, String>>()?
        .join("&");
    let client = HttpClient::new().map_err(e)?;
    client.DefaultRequestHeaders().map_err(e)?.UserAgent().map_err(e)?.TryParseAdd(&HSTRING::from(USER_AGENT)).map_err(e)?;
    let content = HttpStringContent::CreateFromStringWithEncodingAndMediaType(
        &HSTRING::from(body),
        UnicodeEncoding::Utf8,
        &HSTRING::from("application/x-www-form-urlencoded"),
    )
    .map_err(e)?;
    let uri = Uri::CreateUri(&HSTRING::from(TOKEN_URL)).map_err(e)?;
    let response = client.PostAsync(&uri, &content).map_err(e)?.join().map_err(e)?;
    let text = response.Content().map_err(e)?.ReadAsStringAsync().map_err(e)?.join().map_err(e)?.to_string();
    let json: Value = serde_json::from_str(&text).map_err(|_| format!("resposta inesperada do Discord: {text}"))?;
    let access = json["access_token"].as_str().ok_or_else(|| {
        // Ex.: {"error":"invalid_client"} → client id/secret errados.
        format!("o Discord não deu token: {}", json["error_description"].as_str().or(json["error"].as_str()).unwrap_or("?"))
    })?;
    Ok(DiscordToken { access: access.to_string(), refresh: json["refresh_token"].as_str().map(str::to_string) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn membro_usa_apelido_depois_nome_global_depois_usuario() {
        let v = json!({
            "nick": "",
            "user": { "id": "1", "username": "naoki", "global_name": "Naoki", "avatar": "abc" },
            "voice_state": { "self_mute": true, "deaf": false }
        });
        let m = parse_member(&v).unwrap();
        assert_eq!(m.name, "Naoki");
        assert!(m.muted && !m.deafened);
        assert_eq!(m.avatar_url.as_deref(), Some("https://cdn.discordapp.com/avatars/1/abc.png?size=64"));

        let v = json!({ "nick": "Nick", "user": { "id": "2", "username": "u", "avatar": null }, "voice_state": {} });
        let m = parse_member(&v).unwrap();
        assert_eq!(m.name, "Nick");
        assert_eq!(m.avatar_url, None);
    }

    #[test]
    fn quadro_vai_e_volta_pelo_formato_do_ipc() {
        // 8 bytes de cabeçalho (op + tamanho, little-endian) e o JSON.
        let payload = json!({ "v": 1, "client_id": "123" });
        let body = serde_json::to_vec(&payload).unwrap();
        let mut buf = Vec::new();
        buf.extend_from_slice(&OP_HANDSHAKE.to_le_bytes());
        buf.extend_from_slice(&(body.len() as u32).to_le_bytes());
        buf.extend_from_slice(&body);
        assert_eq!(&buf[..4], &[0, 0, 0, 0]);
        assert_eq!(u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize, body.len());
    }
}
