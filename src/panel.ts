import "./lib/dev-mock"; // só em dev no navegador (ver o arquivo)
import { applyAccent } from "./lib/accent";
import { initI18n, t } from "./lib/i18n";
import { armAngle, PANEL_ARM, setArm } from "./lib/arm";
import { setCover } from "./lib/disc";
import { initDiskFx } from "./lib/fx";
import {
  getLyrics,
  getState,
  getVoice,
  isPanelFree,
  minimizePanel,
  next,
  onLyrics,
  onNowPlaying,
  onPanelFree,
  onVoice,
  togglePanelSize,
  onPanelVisible,
  previous,
  seek,
  togglePlayPause,
} from "./lib/ipc";
import { LyricsView, type Lyrics } from "./lib/lyrics";
import { OptionsPanel } from "./lib/options";
import { Shelf } from "./lib/shelf";
import { VoiceList } from "./lib/voice";
import { SeekBar } from "./lib/seek";
import { shareNowPlaying, shareShelf, type ShareResult } from "./lib/share";
import { Crackle, initVinyl, type CrackleLevel } from "./lib/vinyl";
import { VolumeControl } from "./lib/volume";
import { livePositionMs, type NowPlaying } from "./lib/state";

await initI18n(); // idioma antes de qualquer texto desenhado pelo código

const $ = (id: string) => document.getElementById(id)!;
const el = {
  stage: $("stage"),
  arm: $("arm"),
  cover: $("cover"),
  status: $("status"),
  title: $("title"),
  artist: $("artist"),
  album: $("album"),
  prev: $("btn-prev") as HTMLButtonElement,
  play: $("btn-play") as HTMLButtonElement,
  next: $("btn-next") as HTMLButtonElement,
};

const lyricsView = new LyricsView(
  $("lyrics"),
  Array.from(document.querySelectorAll<HTMLElement>("#lyrics .s")),
);

const STATUS_LABEL = {
  playing: t("TOCANDO AGORA"),
  paused: t("PAUSADO"),
  stopped: t("PARADO"),
} as const;

const REF_W = 1280;
const REF_H = 720;

let current: NowPlaying | null = null;
let timer: number | undefined;
/** O Rust escondeu a janela (o WebView nem sempre vira `document.hidden`). */
let hiddenByHost = false;

const isActive = () => !document.hidden && !hiddenByHost;

/** Chiado de vinil (opcional): só soa tocando e com a janela visível. */
const crackle = new Crackle();

/** Pausado por tanto tempo, o disco volta para a capa. */
const SLEEVE_AFTER_MS = 45_000;
let pausedSince = 0;
let sleeveTimer: number | undefined;
function syncSleeve() {
  window.clearTimeout(sleeveTimer);
  const s = current;
  if (!s || s.status === "playing" || !s.title) {
    document.body.removeAttribute("data-sleeved");
    return;
  }
  if (!isActive()) return; // janela escondida: sem timers; decide quando voltar a aparecer
  const left = SLEEVE_AFTER_MS - (Date.now() - pausedSince);
  if (left <= 0) document.body.setAttribute("data-sleeved", "");
  else sleeveTimer = window.setTimeout(syncSleeve, left);
}
let crackleLevel: CrackleLevel = "off";
let crackleRpm = 33;
const syncCrackle = () => crackle.sync(crackleLevel, current?.status === "playing" && isActive(), crackleRpm);

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

/** Pular: o relógio muda na hora; a fonte confirma (ou recusa) depois. */
function applySeek(positionMs: number) {
  if (!current) return;
  current = { ...current, positionMs, sampledAtMs: Date.now() };
  renderTime();
  renderArm(true);
  syncLyrics();
}

/** Linha de duração (clicar/arrastar). */
const seekBar = new SeekBar(
  $("seek"),
  $("seek-bar") as HTMLInputElement,
  $("seek-pos"),
  $("seek-dur"),
  applySeek,
);

function renderTime() {
  seekBar.render(current); // sem duração, a linha some
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
const LYRICS_KEY = "vitrolinha.lyricsHidden";
let lyricsHidden = false;
try {
  lyricsHidden = localStorage.getItem(LYRICS_KEY) === "1";
} catch {
  /* sem armazenamento: começa visível */
}

function applyLyricsHidden() {
  document.body.dataset.lyrics = lyricsHidden ? "off" : "on";
  const btn = document.getElementById("btn-lyrics")!;
  const label = t(lyricsHidden ? "Mostrar letra" : "Ocultar letra");
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
  if (!prev || prev.status !== s.status || trackChanged) pausedSince = Date.now();
  if (trackChanged && prev && isActive()) {
    // Reinicia a animação de entrada do texto e da capa.
    document.body.classList.remove("track-in");
    void document.body.offsetWidth;
    document.body.classList.add("track-in");
  }
  if (trackChanged) lyricsView.setLyrics(null); // a da faixa nova chega pelo evento `lyrics`

  document.body.dataset.status = s.status;
  el.status.textContent = STATUS_LABEL[s.status];
  el.title.textContent = s.title || "—";
  el.title.className = titleClass(s.title);
  el.artist.textContent = s.artist;
  el.album.textContent = s.album;
  el.play.setAttribute("aria-label", t(s.status === "playing" ? "Pausar" : "Tocar"));
  // Sem sessão de mídia (parado e sem título) não há o que controlar.
  const hasSession = s.status !== "stopped" || s.title !== "";
  el.prev.disabled = el.play.disabled = el.next.disabled = !hasSession;
  (document.getElementById("btn-share") as HTMLButtonElement).disabled = !hasSession;
  setCover(el.cover, s.artworkPath);
  $("jacket-cover").style.backgroundImage = el.cover.style.backgroundImage;
  applyAccent(s.accentColor);
  renderTime();
  renderArm(sweep);
  syncTimer();
  syncLyrics(); // play/pause/seek mudam a posição ou o agendamento
  syncCrackle();
  syncSleeve();
}

/** Janela escondida: sem timer e sem animação (CSS via body[data-hidden]). */
function syncActive() {
  document.body.toggleAttribute("data-hidden", !isActive());
  syncVolume();
  syncCrackle();
  syncSleeve();
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
const shelf = new Shelf($("shelf"), $("shelf-grid"), $("shelf-count"), $("btn-shelf"));

/** Aviso rápido no pé da janela. */
let toastTimer: number | undefined;
function toast(text: string) {
  const el = $("toast");
  el.textContent = text;
  el.hidden = false;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => (el.hidden = true), 5000);
}
/** Cartões: gera a imagem, salva em Imagens\Vitrolinha e copia para a área de transferência. */
async function makeCard(button: HTMLButtonElement, build: () => Promise<ShareResult>) {
  button.disabled = true;
  try {
    const r = await build();
    const where = r.path ? "salvo em Imagens\\Vitrolinha" : "não foi possível salvar o arquivo";
    toast(r.copied ? `Cartão copiado e ${where}.` : `Cartão ${where}.`);
  } catch (e) {
    console.warn(e);
    toast(t("Não foi possível gerar o cartão."));
  } finally {
    button.disabled = false;
  }
}
const shareBtn = $("btn-share") as HTMLButtonElement;
shareBtn.addEventListener("click", () => current && void makeCard(shareBtn, () => shareNowPlaying(current!)));
const shelfShareBtn = $("shelf-share") as HTMLButtonElement;
shelfShareBtn.addEventListener("click", () => void makeCard(shelfShareBtn, () => shareShelf(shelf.records)));
$("btn-shelf").addEventListener("click", () => shelf.toggle());

/** Opções: o menu da bandeja desenhado aqui. Abrir uma fecha a outra (as duas cobrem a janela). */
const options = new OptionsPanel($("options"), $("options-body"), $("btn-menu"), () => shelf.close());
$("btn-menu").addEventListener("click", () => options.toggle());
$("btn-shelf").addEventListener("click", () => options.isOpen() && options.close());
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
  const label = t(free ? "Ocupar o monitor inteiro" : "Diminuir janela");
  size.setAttribute("aria-label", label);
  size.title = label;
}
void onPanelFree(applyFree);
void isPanelFree().then(applyFree);

addEventListener("resize", fit);
initDiskFx();
initVinyl((v) => {
  crackleLevel = v.crackle;
  crackleRpm = v.rpm;
  syncCrackle();
});
fit();
syncVolume();

// Call do Discord: o Rust manda o estado inteiro a cada mudança (some em janela livre, via CSS).
const voiceList = new VoiceList($("voice"));
void onVoice((v) => voiceList.render(v));
void getVoice().then((v) => voiceList.render(v)).catch(() => {});
if (import.meta.env.DEV) (window as unknown as { __voice: VoiceList }).__voice = voiceList; // teste no navegador

void onNowPlaying(render);
void onLyrics(applyLyrics);
void getState().then((s) => {
  if (s && !current) render(s); // estado emitido antes de o front estar pronto
  void getLyrics().then(applyLyrics); // idem para a letra
});
