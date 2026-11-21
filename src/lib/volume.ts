// Controle de volume do painel (volume do sistema). Sem timers próprios: o painel
// chama `refresh()` quando está visível e a cada poucos segundos (ver panel.ts).
import { getVolume, setVolume, toggleMute, type Volume } from "./ipc";

const WHEEL_STEP = 0.02;

export class VolumeControl {
  private dragging = false;
  private pending: number | undefined; // próximo valor a enviar (agrupa o arrastar)
  private sending = false;

  constructor(
    private readonly root: HTMLElement,
    private readonly slider: HTMLInputElement,
    muteButton: HTMLButtonElement,
  ) {
    slider.addEventListener("pointerdown", () => (this.dragging = true));
    addEventListener("pointerup", () => (this.dragging = false));
    slider.addEventListener("input", () => {
      this.paint(Number(slider.value) / 100, false);
      this.send(Number(slider.value) / 100);
    });
    // Roda do mouse sobre o controle: ±2% por clique.
    root.addEventListener(
      "wheel",
      (e) => {
        e.preventDefault();
        const next = Math.min(1, Math.max(0, Number(slider.value) / 100 + (e.deltaY < 0 ? WHEEL_STEP : -WHEEL_STEP)));
        this.paint(next, false);
        this.send(next);
      },
      { passive: false },
    );
    muteButton.addEventListener("click", () => void toggleMute().then((v) => this.apply(v)).catch(() => {}));
  }

  /** Lê o volume atual (ignora enquanto o usuário arrasta, para não brigar com ele). */
  refresh() {
    if (this.dragging || this.sending) return;
    void getVolume().then((v) => this.apply(v)).catch(() => {});
  }

  private apply(v: Volume | null) {
    this.root.hidden = v === null; // sem dispositivo de saída: some
    if (v && !this.dragging) this.paint(v.level, v.muted);
  }

  private paint(level: number, muted: boolean) {
    const pct = Math.round(level * 100);
    this.slider.value = String(pct);
    this.slider.style.setProperty("--v", `${muted ? 0 : pct}%`);
    this.slider.setAttribute("aria-valuetext", muted ? "mudo" : `${pct}%`);
    document.body.toggleAttribute("data-muted", muted || pct === 0);
  }

  /** Um envio por vez: durante o arrastar só vale o último valor. */
  private send(level: number) {
    this.pending = level;
    if (this.sending) return;
    this.sending = true;
    const loop = () => {
      const value = this.pending;
      this.pending = undefined;
      if (value === undefined) {
        this.sending = false;
        return;
      }
      void setVolume(value)
        .catch(() => null)
        .then(loop);
    };
    loop();
  }
}
