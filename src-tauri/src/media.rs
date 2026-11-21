//! GSMTC: observa a sessão de mídia atual do Windows e emite `now-playing`.
//!
//! Roda numa thread própria (MTA) para poder bloquear em operações WinRT
//! sem travar a thread da UI. Sem polling: tudo é dirigido por eventos.

use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::artwork::ArtCache;
use crate::{lyrics, popup, settings};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use windows::core::Result;
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as SessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
};
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

/// Agrupa rajadas de eventos (uma troca de faixa dispara vários, alguns com
/// metadados velhos).
const DEBOUNCE: Duration = Duration::from_millis(250);
/// Só reemite por mudança de timeline se a posição real fugir da prevista
/// (seek, ou fonte que corrige o relógio) por mais que isso.
const DRIFT_MS: i64 = 2_000;

/// Contrato Rust → front (espelha `NowPlaying` em src/lib/state.ts).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub status: &'static str, // "playing" | "paused" | "stopped"
    pub position_ms: i64,
    pub duration_ms: i64,
    pub sampled_at_ms: i64,
    pub artwork_path: Option<String>, // JPEG temporário ~512px, ver artwork.rs
    /// Cor de destaque da capa ("#rrggbb"), ver accent.rs. `None` = usar o âmbar padrão.
    pub accent_color: Option<String>,
    pub source_app: String,
}

impl NowPlaying {
    fn stopped() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            status: "stopped",
            position_ms: 0,
            duration_ms: 0,
            sampled_at_ms: now_ms(),
            artwork_path: None,
            accent_color: None,
            source_app: String::new(),
        }
    }

    /// Posição estimada em `at_ms`, como o front faz.
    fn position_at(&self, at_ms: i64) -> i64 {
        let elapsed = if self.status == "playing" { (at_ms - self.sampled_at_ms).max(0) } else { 0 };
        let pos = self.position_ms + elapsed;
        if self.duration_ms > 0 { pos.min(self.duration_ms) } else { pos }
    }

    /// Mesma faixa (a capa e o tempo podem mudar sem a faixa mudar).
    fn same_track(&self, other: &NowPlaying) -> bool {
        self.title == other.title
            && self.artist == other.artist
            && self.album == other.album
            && self.source_app == other.source_app
    }

    /// Vale a pena avisar o front?
    fn differs_from(&self, prev: &NowPlaying) -> bool {
        self.title != prev.title
            || self.artist != prev.artist
            || self.album != prev.album
            || self.status != prev.status
            || self.source_app != prev.source_app
            || self.duration_ms != prev.duration_ms
            || self.artwork_path != prev.artwork_path
            || (prev.position_at(self.sampled_at_ms) - self.position_ms).abs() > DRIFT_MS
    }
}

#[derive(Default)]
pub struct SharedState {
    /// Última amostra emitida (para `get_state`).
    pub now: Mutex<Option<NowPlaying>>, // lido também por lyrics::reload
    /// Gerenciador GSMTC, para os comandos acharem a sessão atual.
    manager: Mutex<Option<SessionManager>>,
    /// App de mídia escolhido como única fonte (AUMID); `None` = o que o sistema
    /// considerar "atual". Os controles (tocar/pausar...) agem nessa mesma sessão.
    source: Mutex<Option<String>>,
    /// Acorda o loop principal (a fonte mudou).
    wake: Mutex<Option<Sender<()>>>,
}

/// Sessão a seguir: a do app escolhido, ou a "atual" do sistema.
/// Navegadores abrem sessões próprias (vídeos de outras abas) e o sistema as
/// elege "atual" quando o player principal pausa: com `source`, isso não acontece.
fn pick_session(manager: &SessionManager, source: Option<&str>) -> Result<Session> {
    let Some(id) = source else { return manager.GetCurrentSession() };
    for session in manager.GetSessions()? {
        if session.SourceAppUserModelId()?.to_string_lossy() == id {
            return Ok(session);
        }
    }
    Err(windows::core::Error::new(windows::core::HRESULT(0x8000_4005_u32 as i32), "app de mídia escolhido não está aberto"))
}

/// Uma fonte de mídia (app) com sessão ativa agora.
pub struct Source {
    pub id: String,
    pub label: String,
}

/// Apps com sessão de mídia no momento (para o menu "Fonte de mídia").
pub fn sources(app: &AppHandle) -> Vec<Source> {
    let manager = app.state::<SharedState>().manager.lock().unwrap().clone();
    let Some(sessions) = manager.and_then(|m| m.GetSessions().ok()) else {
        return Vec::new();
    };
    let mut out: Vec<Source> = Vec::new();
    for s in sessions {
        if let Ok(id) = s.SourceAppUserModelId() {
            let id = id.to_string();
            if !out.iter().any(|o| o.id == id) {
                out.push(Source { label: friendly_name(&id), id });
            }
        }
    }
    out
}

/// Escolhe a única fonte (`None` = qualquer) e reavalia na hora.
pub fn set_source(app: &AppHandle, source: Option<String>) {
    settings::set_media_source(app, source.as_deref());
    let state = app.state::<SharedState>();
    *state.source.lock().unwrap() = source;
    if let Some(tx) = state.wake.lock().unwrap().as_ref() {
        let _ = tx.send(());
    }
}

/// Nome legível a partir do AUMID ("music.youtube.com-XXXX!App" → "YouTube Music").
pub fn friendly_name(aumid: &str) -> String {
    let lower = aumid.to_lowercase();
    let known = [
        ("music.youtube.com", "YouTube Music"),
        ("spotify", "Spotify"),
        ("operagx", "Opera GX (navegador)"),
        ("opera", "Opera (navegador)"),
        ("chrome", "Chrome (navegador)"),
        ("msedge", "Edge (navegador)"),
        ("firefox", "Firefox (navegador)"),
        ("vlc", "VLC"),
    ];
    if let Some((_, name)) = known.iter().find(|(k, _)| lower.contains(k)) {
        return (*name).to_string();
    }
    // Resto: primeiro trecho antes de '!' ou '_' (nome do pacote), sem ".exe".
    let base = aumid.split(['!', '_']).next().unwrap_or(aumid);
    base.trim_end_matches(".exe").to_string()
}

#[derive(Clone, Copy, Debug)]
pub enum Control {
    TogglePlayPause,
    Next,
    Previous,
}

/// Manda um comando de transporte à sessão atual do sistema.
/// A espera pelo resultado fica numa thread curta (MTA) para não bloquear a UI;
/// o efeito chega ao front pelos eventos normais de `now-playing`.
pub fn control(state: &SharedState, what: Control) -> std::result::Result<(), String> {
    let manager = state.manager.lock().unwrap().clone().ok_or("mídia ainda não iniciada")?;
    let source = state.source.lock().unwrap().clone();
    let session = pick_session(&manager, source.as_deref()).map_err(|_| "nenhuma sessão de mídia ativa")?;
    thread::spawn(move || {
        let result = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
            .and_then(|_| match what {
                Control::TogglePlayPause => session.TryTogglePlayPauseAsync(),
                Control::Next => session.TrySkipNextAsync(),
                Control::Previous => session.TrySkipPreviousAsync(),
            }?
            .join());
        match result {
            Ok(true) => {}
            Ok(false) => eprintln!("[media] a fonte recusou {what:?}"),
            Err(e) => eprintln!("[media] falha em {what:?}: {e}"),
        }
    });
    Ok(())
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// FILETIME-style ticks (100 ns desde 1601) → epoch ms.
fn ticks_to_epoch_ms(ticks: i64) -> i64 {
    (ticks - 116_444_736_000_000_000) / 10_000
}

fn status_str(s: PlaybackStatus) -> &'static str {
    match s {
        PlaybackStatus::Playing => "playing",
        PlaybackStatus::Paused => "paused",
        _ => "stopped",
    }
}

/// Lê tudo da sessão. Posição já corrigida para "agora":
/// pos = Position + (agora − LastUpdatedTime) enquanto toca.
///
/// A capa só é decodificada quando a faixa muda (título/artista/álbum/app);
/// enquanto for a mesma faixa reaproveita a anterior.
fn sample(session: &Session, last: Option<&NowPlaying>, art: &mut ArtCache) -> Result<NowPlaying> {
    let props = session.TryGetMediaPropertiesAsync()?.join()?;
    let status = status_str(session.GetPlaybackInfo()?.PlaybackStatus()?);
    let tl = session.GetTimelineProperties()?;

    // TimeSpan.Duration: unidades de 100 ns.
    let start = tl.StartTime()?.Duration / 10_000;
    let end = tl.EndTime()?.Duration / 10_000;
    let duration_ms = (end - start).max(0);
    let mut position_ms = (tl.Position()?.Duration / 10_000 - start).max(0);

    let now = now_ms();
    if status == "playing" {
        let age = now - ticks_to_epoch_ms(tl.LastUpdatedTime()?.UniversalTime);
        // Fontes que não preenchem LastUpdatedTime dão idade absurda: ignora.
        if (0..12 * 3_600_000).contains(&age) {
            position_ms += age;
        }
    }
    if duration_ms > 0 {
        position_ms = position_ms.min(duration_ms);
    }

    let title = props.Title()?.to_string();
    let artist = props.Artist()?.to_string();
    let album = props.AlbumTitle()?.to_string();
    let source_app = session.SourceAppUserModelId()?.to_string();

    let cached = last
        .filter(|p| {
            p.title == title && p.artist == artist && p.album == album && p.source_app == source_app
        })
        .and_then(|p| p.artwork_path.clone().map(|path| (path, p.accent_color.clone())));
    // Sem capa em cache → tenta (inclui a faixa que ainda não entregou o thumbnail).
    let (artwork_path, accent_color) = match cached {
        Some((path, accent)) => (Some(path), accent),
        None => match art.extract(&props) {
            Some(cover) => (Some(cover.path), cover.accent),
            None => (None, None),
        },
    };

    Ok(NowPlaying {
        title,
        artist,
        album,
        status,
        position_ms,
        duration_ms,
        sampled_at_ms: now,
        artwork_path,
        accent_color,
        source_app,
    })
}

/// Handler que só avisa o loop principal (os tipos de sender/args variam por
/// evento, por isso macro em vez de função).
macro_rules! notify {
    ($tx:expr) => {{
        let tx = $tx.clone();
        TypedEventHandler::new(move |_, _| {
            let _ = tx.send(());
            Ok(())
        })
    }};
}

/// Assinaturas nos eventos de uma sessão; removidas ao trocar de sessão.
struct Subscription {
    session: Session,
    media: i64,
    playback: i64,
    timeline: i64,
}

impl Subscription {
    fn new(session: Session, tx: &Sender<()>) -> Result<Self> {
        let media = session.MediaPropertiesChanged(&notify!(tx))?;
        let playback = session.PlaybackInfoChanged(&notify!(tx))?;
        let timeline = session.TimelinePropertiesChanged(&notify!(tx))?;
        Ok(Self { session, media, playback, timeline })
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let _ = self.session.RemoveMediaPropertiesChanged(self.media);
        let _ = self.session.RemovePlaybackInfoChanged(self.playback);
        let _ = self.session.RemoveTimelinePropertiesChanged(self.timeline);
    }
}

pub fn start(app: AppHandle) {
    thread::Builder::new()
        .name("gsmtc".into())
        .spawn(move || {
            if let Err(e) = run(&app) {
                eprintln!("[media] encerrado com erro: {e}");
            }
        })
        .expect("não foi possível criar a thread de mídia");
}

fn publish(app: &AppHandle, np: NowPlaying) {
    println!(
        "[media] {:<7} {} — {} ({}) {}/{}ms [{}]",
        np.status, np.title, np.artist, np.album, np.position_ms, np.duration_ms, np.source_app
    );
    *app.state::<SharedState>().now.lock().unwrap() = Some(np.clone());
    crate::history::observe(app, &np); // estante: conta a faixa e guarda a capa
    emit(app, &np);
}

fn emit(app: &AppHandle, np: &NowPlaying) {
    if let Err(e) = app.emit("now-playing", &for_ui(app, np)) {
        eprintln!("[media] falha ao emitir: {e}");
    }
}

/// O que o front recebe: com "Cor da capa" desligada, sem `accentColor`.
/// (O estado guardado mantém a cor, para religar sem reler a capa.)
pub fn for_ui(app: &AppHandle, np: &NowPlaying) -> NowPlaying {
    let mut out = np.clone();
    if !settings::dynamic_color(app) {
        out.accent_color = None;
    }
    crate::hdcover::apply(app, &mut out);
    out
}

/// Reemite o estado atual (ex.: depois de mudar uma configuração visual).
pub fn refresh(app: &AppHandle) {
    let current = app.state::<SharedState>().now.lock().unwrap().clone();
    if let Some(np) = current {
        emit(app, &np);
    }
}

fn run(app: &AppHandle) -> Result<()> {
    unsafe { RoInitialize(RO_INIT_MULTITHREADED)? };
    let manager = SessionManager::RequestAsync()?.join()?;
    let (tx, rx) = channel::<()>();
    {
        let state = app.state::<SharedState>();
        *state.manager.lock().unwrap() = Some(manager.clone());
        *state.source.lock().unwrap() = settings::media_source(app);
        *state.wake.lock().unwrap() = Some(tx.clone());
    }

    // Mudou a sessão "atual" ou a lista de sessões → reavaliar.
    manager.CurrentSessionChanged(&notify!(tx))?;
    manager.SessionsChanged(&notify!(tx))?;

    let mut art = ArtCache::new();
    let mut subscription: Option<Subscription> = None;
    let mut last: Option<NowPlaying> = None;
    let mut first = true;

    loop {
        if !first {
            if rx.recv().is_err() {
                return Ok(());
            }
            loop {
                match rx.recv_timeout(DEBOUNCE) {
                    Ok(()) => continue,
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return Ok(()),
                }
            }
        }
        first = false;

        let source = app.state::<SharedState>().source.lock().unwrap().clone();
        let next = match pick_session(&manager, source.as_deref()) {
            Ok(session) => {
                let same = subscription.as_ref().is_some_and(|s| {
                    s.session.SourceAppUserModelId().ok() == session.SourceAppUserModelId().ok()
                });
                if !same {
                    drop(subscription.take()); // remove as assinaturas antigas
                    subscription = Some(Subscription::new(session.clone(), &tx)?);
                }
                match sample(&session, last.as_ref(), &mut art) {
                    Ok(np) => np,
                    Err(e) => {
                        eprintln!("[media] erro ao ler a sessão: {e}");
                        continue;
                    }
                }
            }
            Err(_) => {
                drop(subscription.take());
                NowPlaying::stopped()
            }
        };

        if last.as_ref().map_or(true, |prev| next.differs_from(prev)) {
            // O popup só aparece quando já havia um estado anterior (não na
            // primeira leitura) e há título para mostrar.
            let show_popup = !next.title.is_empty()
                && last.as_ref().is_some_and(|prev| {
                    if !next.same_track(prev) {
                        true // faixa nova
                    } else {
                        next.status != prev.status && settings::popup_on_play_pause(app)
                    }
                });
            // Faixa nova (inclui a primeira leitura): busca a letra / limpa a anterior.
            let new_track = last.as_ref().map_or(true, |prev| !next.same_track(prev));
            let for_lyrics = new_track.then(|| next.clone());
            last = Some(next.clone());
            publish(app, next);
            if let Some(np) = for_lyrics {
                lyrics::request(app, &np);
                // Capa grande, se a do player for pequena (resultado chega depois, por evento).
                crate::hdcover::request(app, &np, art.short_side());
            }
            if show_popup {
                popup::trigger(app); // depois do now-playing: o front já tem o texto novo
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::friendly_name;

    #[test]
    fn names_known_apps() {
        assert_eq!(friendly_name("music.youtube.com-5929F88E_vezhnr0wkvrcy!App"), "YouTube Music");
        assert_eq!(friendly_name("OperaSoftware.OperaGXWebBrowser.1756016046"), "Opera GX (navegador)");
        assert_eq!(friendly_name("Spotify.exe"), "Spotify");
    }

    #[test]
    fn unknown_apps_use_the_package_name() {
        assert_eq!(friendly_name("Contoso.Player_abc123!App"), "Contoso.Player");
        assert_eq!(friendly_name("meuplayer.exe"), "meuplayer");
    }
}
