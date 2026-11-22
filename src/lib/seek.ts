// Linha de duração do painel: mostra a posição e deixa o usuário escolher para onde pular.
// Sem timers próprios: o painel chama `render()` a cada tick de 1 s (ver panel.ts).
import { seek } from "./ipc";
import { formatTime, livePositionMs, type NowPlaying } from "./state";

export class SeekBar {
  private dragging = false;

  /**
   * @param onSeek chamado ao soltar/confirmar, com a posição pedida (ms): o painel atualiza
   *               o relógio na hora, sem esperar a fonte responder.
   */
  constructor(
    private readonly root: HTMLElement,
    private readonly bar: HTMLInputElement,
    private readonly pos: HTMLElement,
    private readonly dur: HTMLElement,
    private readonly onSeek: (positionMs: number) => void,
  ) {
    bar.addEventListener("pointerdown", () => (this.dragging = true));
    addEventListener("pointerup", () => (this.dragging = false));
    addEventListener("pointercancel", () => (this.dragging = false));
    // Arrastando: só a prévia (rótulo e preenchimento). O pulo vem ao soltar.
    bar.addEventListener("input", () => this.paint(Number(bar.value)));
    bar.addEventListener("change", () => {
      const ms = Number(bar.value) * 1000;
      this.onSeek(ms);
      void seek(ms).catch((e) => console.warn(e));
    });
  }

  render(s: NowPlaying | null) {
    // Sem duração (transmissão ao vivo, fonte que não informa): nada para mostrar.
    const visible = !!s && s.durationMs > 0;
    this.root.hidden = !visible;
    if (!visible) return;

    const total = Math.ceil(s.durationMs / 1000);
    this.bar.max = String(total);
    this.bar.disabled = !s.canSeek;
    this.root.toggleAttribute("data-readonly", !s.canSeek);
    this.dur.textContent = formatTime(s.durationMs);
    if (!this.dragging) this.paint(Math.floor(livePositionMs(s) / 1000), true);
  }

  private paint(seconds: number, fromClock = false) {
    const total = Number(this.bar.max) || 1;
    if (fromClock) this.bar.value = String(seconds);
    this.bar.style.setProperty("--v", `${Math.min(100, (seconds / total) * 100)}%`);
    this.bar.setAttribute("aria-valuetext", `${formatTime(seconds * 1000)} de ${this.dur.textContent}`);
    this.pos.textContent = formatTime(seconds * 1000);
  }
}
