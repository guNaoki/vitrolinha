import { applyAccent } from "./lib/accent";
import { armAngle, PANEL_ARM, setArm } from "./lib/arm";
import { setCover } from "./lib/disc";
import { initDiskFx } from "./lib/fx";
import {
  getLyrics,
  getState,
  isPanelFree,
  minimizePanel,
  next,
  onLyrics,
  onNowPlaying,
  onPanelFree,
  togglePanelSize,
  onPanelVisible,
  previous,
  togglePlayPause,
} from "./lib/ipc";
import { LyricsView, type Lyrics } from "./lib/lyrics";
import { VolumeControl } from "./lib/volume";
import { formatTime, livePositionMs, type NowPlaying } from "./lib/state";

const $ = (id: string) => document.getElementById(id)!;
const el = {
  stage: $("stage"),
  arm: $("arm"),
  cover: $("cover"),
  status: $("status"),
  title: $("title"),
  artist: $("artist"),
  album: $("album"),
  time: $("time"),
  prev: $("btn-prev") as HTMLButtonElement,
  play: $("btn-play") as HTMLButtonElement,
  next: $("btn-next") as HTMLButtonElement,
};

const lyricsView = new LyricsView(
  $("lyrics"),
  Array.from(document.querySelectorAll<HTMLElement>("#lyrics .s")),
);

const STATUS_LABEL = {
  playing: "TOCANDO AGORA",
  paused: "PAUSADO",
  stopped: "PARADO",
} as const;

const REF_W = 1280;
const REF_H = 720;

let current: NowPlaying | null = null;
let timer: number | undefined;
/** O Rust escondeu a janela (o WebView nem sempre vira `document.hidden`). */
let hiddenByHost = false;

const isActive = () => !document.hidden && !hiddenByHost;

/** Abaixo disso (largura/altura) o layout vira "alto": monitor vertical, quadrado... */
const TALL_BELOW = 1.2;

/**
 * Ajusta o palco à janela (qualquer resolução/DPI).
 * - Janela larga: palco 1280×720 escalado para caber, centralizado.
 * - Janela alta: escala pela largura e o palco ganha altura (--stage-h), em vez
 *   de virar uma faixa 16:9 no meio da tela.
 */
function fit() {
  // Moldura de toca-discos (janela livre): sempre o layout largo, dentro da moldura.
  const deck = document.documentElement.dataset.frame === "deck";
  const tall = !deck && innerWidth / innerHeight < TALL_BELOW;
  const bezel = deck ? Math.max(10, Math.min(24, Math.round(Math.min(innerWidth, innerHeight) * 0.035))) : 0;
  const pad = deck ? bezel + 6 : 0; // folga entre a borda da placa e o palco
  const availW = innerWidth - 2 * pad;
  const availH = innerHeight - 2 * pad;
  const s = tall ? innerWidth / REF_W : Math.min(availW / REF_W, availH / REF_H);
  const stageH = tall ? innerHeight / s : REF_H;
  const root = document.documentElement.style;
  root.setProperty("--s", String(s));
  root.setProperty("--stage-h", `${stageH}px`);
  root.setProperty("--bezel", `${bezel}px`);
  root.setProperty("--tx", `${tall ? 0 : pad + (availW - REF_W * s) / 2}px`);
  root.setProperty("--ty", `${tall ? 0 : pad + (availH - REF_H * s) / 2}px`);
  document.body.dataset.layout = tall ? "tall" : "wide";
}

function renderTime() {
  const s = current;
  if (!s || s.durationMs <= 0) {
    el.time.textContent = ""; // sem duração → tempo escondido
    return;
  }
  el.time.textContent = `${formatTime(livePositionMs(s))} / ${formatTime(s.durationMs)}`;
}

function renderArm(sweep: boolean) {
  const s = current;
  if (!s || s.status !== "playing") {
    setArm(el.arm, PANEL_ARM.rest, true, sweep);
    return;
  }
  // Sem duração → braço parado na posição inicial.
  const p = s.durationMs > 0 ? livePositionMs(s) / s.durationMs : 0;
  setArm(el.arm, armAngle(p, PANEL_ARM), false, sweep);
}

/** Letra oculta pelo botão (só visual; lembrado entre execuções). */
const LYRICS_KEY = "flyout.lyricsHidden";
let lyricsHidden = false;
try {
  lyricsHidden = localStorage.getItem(LYRICS_KEY) === "1";
} catch {
  /* sem armazenamento: começa visível */
}

function applyLyricsHidden() {
  document.body.dataset.lyrics = lyricsHidden ? "off" : "on";
  const btn = document.getElementById("btn-lyrics")!;
  const label = lyricsHidden ? "Mostrar letra" : "Ocultar letra";
  btn.setAttribute("aria-label", label);
  btn.setAttribute("aria-pressed", String(lyricsHidden));
  btn.title = label;
}

/** Letra acompanhando a posição: só agenda troca de linha tocando e com a janela visível. */
function syncLyrics() {
  const s = current;
  if (!s) return;
  if (!isActive() || lyricsHidden) {
    lyricsView.stop();
    return;
  }
  lyricsView.sync(() => livePositionMs(s), s.status === "playing");
}

/** Aceita a letra só se for da faixa que está na tela. */
function applyLyrics(l: Lyrics | null) {
  if (l && current && l.title === current.title && l.artist === current.artist) {
    lyricsView.setLyrics(l.lines);
    syncLyrics();
  } else if (!l) {
    lyricsView.setLyrics(null);
  }
}

function titleClass(title: string): string {
  // A coluna de texto tem ~222px: títulos maiores encolhem em vez de estourar.
  if (title.length > 32) return "longer";
  if (title.length > 14) return "long";
  return "";
}

const volume = new VolumeControl(
  $("volume"),
  $("vol") as HTMLInputElement,
  $("btn-mute") as HTMLButtonElement,
);
/** Volume pode mudar fora daqui (teclas de mídia): relê a cada 2 s, só com a janela visível. */
let volumeTimer: number | undefined;
function syncVolume() {
  window.clearInterval(volumeTimer);
  volumeTimer = undefined;
  if (!isActive()) return;
  volume.refresh();
  volumeTimer = window.setInterval(() => volume.refresh(), 2000);
}

function stopTimer() {
  window.clearInterval(timer);
  timer = undefined;
}

/** 1 tick/s (texto + braço), só enquanto toca e a janela está visível. */
function syncTimer() {
  stopTimer();
  if (current?.status === "playing" && current.durationMs > 0 && isActive()) {
    timer = window.setInterval(() => {
      renderTime();
      renderArm(false);
    }, 1000);
  }
}

function render(s: NowPlaying) {
  const prev = current;
  current = s;
  const trackChanged =
    !prev || prev.title !== s.title || prev.artist !== s.artist || prev.album !== s.album;
  const sweep = trackChanged || prev.status !== s.status;
  if (trackChanged) lyricsView.setLyrics(null); // a da faixa nova chega pelo evento `lyrics`

  document.body.dataset.status = s.status;
  el.status.textContent = STATUS_LABEL[s.status];
  el.title.textContent = s.title || "—";
  el.title.className = titleClass(s.title);
  el.artist.textContent = s.artist;
  el.album.textContent = s.album;
  el.play.setAttribute("aria-label", s.status === "playing" ? "Pausar" : "Tocar");
  // Sem sessão de mídia (parado e sem título) não há o que controlar.
  const hasSession = s.status !== "stopped" || s.title !== "";
  el.prev.disabled = el.play.disabled = el.next.disabled = !hasSession;
  setCover(el.cover, s.artworkPath);
  applyAccent(s.accentColor);
  renderTime();
  renderArm(sweep);
  syncTimer();
  syncLyrics(); // play/pause/seek mudam a posição ou o agendamento
}

/** Janela escondida: sem timer e sem animação (CSS via body[data-hidden]). */
function syncActive() {
  document.body.toggleAttribute("data-hidden", !isActive());
  syncVolume();
  if (!isActive()) {
    stopTimer();
    lyricsView.stop();
  } else if (current) {
    renderTime(); // reconcilia com o relógio após o tempo parado
    renderArm(false);
    syncTimer();
    syncLyrics();
  }
}

document.addEventListener("visibilitychange", syncActive);
void onPanelVisible((visible) => {
  hiddenByHost = !visible;
  syncActive();
});

// Controles: o efeito volta pelo evento `now-playing`.
const run = (fn: () => Promise<void>) => () => void fn().catch((e) => console.warn(e));
el.prev.addEventListener("click", run(previous));
el.play.addEventListener("click", run(togglePlayPause));
el.next.addEventListener("click", run(next));
$("btn-min").addEventListener("click", run(minimizePanel));
$("btn-size").addEventListener("click", run(togglePanelSize));
$("btn-lyrics").addEventListener("click", () => {
  lyricsHidden = !lyricsHidden;
  try {
    localStorage.setItem(LYRICS_KEY, lyricsHidden ? "1" : "0");
  } catch {
    /* tudo bem: vale só nesta execução */
  }
  applyLyricsHidden();
  syncLyrics(); // oculta: para o timer; mostra: volta a acompanhar a posição
});
applyLyricsHidden();

/**
 * Modo de janela livre: a barra de cima arrasta a janela e as bordas redimensionam
 * (no modo "monitor inteiro" a janela fica fixa).
 */
function applyFree(free: boolean) {
  document.body.dataset.mode = free ? "free" : "fill";
  $("chrome").toggleAttribute("data-tauri-drag-region", free);
  // Janela livre = moldura de toca-discos, que também serve de alça de arrastar.
  document.documentElement.dataset.frame = free ? "deck" : "flat";
  $("deck-frame").toggleAttribute("data-tauri-drag-region", free);
  document.querySelector(".deck-plate")!.toggleAttribute("data-tauri-drag-region", free);
  fit();
  const size = $("btn-size");
  const label = free ? "Ocupar o monitor inteiro" : "Diminuir janela";
  size.setAttribute("aria-label", label);
  size.title = label;
}
void onPanelFree(applyFree);
void isPanelFree().then(applyFree);

addEventListener("resize", fit);
initDiskFx();
fit();
syncVolume();

void onNowPlaying(render);
void onLyrics(applyLyrics);
void getState().then((s) => {
  if (s && !current) render(s); // estado emitido antes de o front estar pronto
  void getLyrics().then(applyLyrics); // idem para a letra
});
