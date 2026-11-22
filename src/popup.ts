import { applyAccent } from "./lib/accent";
import { armAngle, POPUP_ARM, setArm } from "./lib/arm";
import { setCover } from "./lib/disc";
import { initDiskFx } from "./lib/fx";
import { initI18n, t } from "./lib/i18n";
import { initVinyl } from "./lib/vinyl";
import { getState, next, onNowPlaying, onPopupLeave, onPopupVisible, previous, togglePlayPause } from "./lib/ipc";
import { formatTime, livePositionMs, type NowPlaying } from "./lib/state";

await initI18n(); // idioma antes de qualquer texto desenhado pelo código

const $ = (id: string) => document.getElementById(id)!;
const el = {
  arm: $("arm"),
  cover: $("cover"),
  title: $("title"),
  artist: $("artist"),
  row: $("progress-row"),
  bar: $("bar"),
  time: $("time"),
  play: $("btn-play"),
};

let current: NowPlaying | null = null;
let timer: number | undefined;
let visible = false; // controlado pelo Rust (popup-visible)

function renderProgress() {
  const s = current;
  const has = !!s && s.durationMs > 0;
  el.row.hidden = !has; // sem duração → barra e tempo escondidos
  if (!s || !has) return;
  const pos = livePositionMs(s);
  el.time.textContent = `${formatTime(pos)} / ${formatTime(s.durationMs)}`;
  el.bar.style.transform = `scaleX(${Math.min(1, pos / s.durationMs).toFixed(4)})`;
}

function renderArm(sweep: boolean) {
  const s = current;
  if (!s || s.status !== "playing") {
    setArm(el.arm, POPUP_ARM.rest, true, sweep);
    return;
  }
  const p = s.durationMs > 0 ? livePositionMs(s) / s.durationMs : 0; // sem duração: início
  setArm(el.arm, armAngle(p, POPUP_ARM), false, sweep);
}

function stopTimer() {
  window.clearInterval(timer);
  timer = undefined;
}

/** 1 tick/s (barra, tempo, braço): só com o popup na tela e tocando. */
function syncTimer() {
  stopTimer();
  if (visible && current?.status === "playing" && current.durationMs > 0) {
    timer = window.setInterval(() => {
      renderProgress();
      renderArm(false);
    }, 1000);
  }
}

function render(s: NowPlaying) {
  const prev = current;
  current = s;
  const sweep = !prev || prev.title !== s.title || prev.status !== s.status;

  document.body.dataset.status = s.status;
  el.title.textContent = s.title || "—";
  el.artist.textContent = s.artist;
  el.play.setAttribute("aria-label", t(s.status === "playing" ? "Pausar" : "Tocar"));
  setCover(el.cover, s.artworkPath);
  applyAccent(s.accentColor);
  renderProgress();
  renderArm(sweep);
  syncTimer();
}

/** O Rust mostrou (true) ou vai esconder (false) a janela. */
function setVisible(v: boolean) {
  visible = v;
  document.body.toggleAttribute("data-hidden", !v);
  if (v) {
    // Reinicia a animação de entrada (também se o popup foi renovado no meio da saída).
    document.body.dataset.state = "idle";
    void document.body.offsetWidth; // força reflow
    document.body.dataset.state = "in";
    renderProgress(); // reconcilia com o relógio
    renderArm(false);
  } else {
    document.body.dataset.state = "idle";
  }
  syncTimer();
}

initDiskFx();
initVinyl(); // rotação 33/45 (o chiado é só do painel)
void onNowPlaying(render);
void onPopupVisible(setVisible);
void onPopupLeave(() => {
  document.body.dataset.state = "out";
});
void getState().then((s) => {
  if (s && !current) render(s);
});

const run = (fn: () => Promise<void>) => () => void fn().catch((e) => console.warn(e));
el.play.addEventListener("click", run(togglePlayPause));
$("btn-prev").addEventListener("click", run(previous));
$("btn-next").addEventListener("click", run(next));
