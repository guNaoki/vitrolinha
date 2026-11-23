// Roda de cores (inspirada no seletor de tema do Zen Browser): um círculo com o matiz em volta e a
// saturação do centro para a borda; arrasta-se uma bolinha para escolher, e um controle de brilho
// ao lado deixa a cor mais escura ou mais clara. Dá prévia ao vivo ao arrastar e só grava ao soltar.
import { t } from "./i18n";

const SIZE = 168; // px (CSS)
const V_MIN = 0.45; // abaixo disso o destaque some no fundo escuro

type Hsv = { h: number; s: number; v: number };

export function hexToHsv(hex: string): Hsv {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const max = Math.max(r, g, b);
  const d = max - Math.min(r, g, b);
  let h = 0;
  if (d) h = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return { h: (h * 60 + 360) % 360, s: max ? d / max : 0, v: max };
}

export function hsvToHex({ h, s, v }: Hsv): string {
  const f = (n: number) => {
    const k = (n + h / 60) % 6;
    return v - v * s * Math.max(0, Math.min(k, 4 - k, 1));
  };
  const hex = (x: number) => Math.round(x * 255).toString(16).padStart(2, "0");
  return `#${hex(f(5))}${hex(f(3))}${hex(f(1))}`;
}

export class ColorWheel {
  readonly el = document.createElement("div");
  private readonly canvas = document.createElement("canvas");
  private readonly dot = document.createElement("i");
  private readonly bright = document.createElement("input");
  private hsv: Hsv;
  private commitTimer: number | undefined;

  /**
   * @param start cor atual (ou a de partida, se nenhuma foi escolhida)
   * @param active se a cor atual foi escolhida à mão (senão a bolinha fica apagada)
   * @param preview chamado enquanto arrasta (`null` = desfaz a prévia)
   * @param commit chamado ao soltar, com a cor final
   */
  constructor(
    start: string,
    active: boolean,
    private readonly preview: (hex: string | null) => void,
    private readonly commit: (hex: string) => void,
  ) {
    this.hsv = hexToHsv(start);
    this.hsv.v = Math.max(V_MIN, this.hsv.v);
    this.el.className = "wheel";
    this.el.toggleAttribute("data-idle", !active);

    this.canvas.className = "wheel-disc";
    this.canvas.width = this.canvas.height = Math.round(SIZE * Math.min(2, window.devicePixelRatio || 1));
    this.canvas.style.width = this.canvas.style.height = `${SIZE}px`;
    this.dot.className = "wheel-dot";
    const disc = document.createElement("div");
    disc.className = "wheel-box";
    disc.tabIndex = 0;
    disc.setAttribute("role", "slider");
    disc.setAttribute("aria-label", t("Roda de cores"));
    disc.append(this.canvas, this.dot);

    this.bright.type = "range";
    this.bright.className = "wheel-bright";
    this.bright.min = String(V_MIN * 100);
    this.bright.max = "100";
    this.bright.setAttribute("aria-label", t("Brilho da cor"));
    this.bright.setAttribute("orient", "vertical");
    this.bright.addEventListener("input", () => {
      this.hsv.v = Number(this.bright.value) / 100;
      this.update(true);
    });
    this.bright.addEventListener("change", () => this.done());

    this.el.append(disc, this.bright);
    this.drawDisc();
    this.update(false);

    // Arrastar (mouse, toque ou caneta).
    const at = (e: PointerEvent) => {
      const r = this.canvas.getBoundingClientRect();
      const x = (e.clientX - r.left) / r.width - 0.5;
      const y = (e.clientY - r.top) / r.height - 0.5;
      const dist = Math.min(0.5, Math.hypot(x, y));
      this.hsv.h = ((Math.atan2(y, x) * 180) / Math.PI + 360) % 360;
      this.hsv.s = dist / 0.5;
      this.update(true);
    };
    disc.addEventListener("pointerdown", (e) => {
      disc.setPointerCapture(e.pointerId);
      at(e);
    });
    disc.addEventListener("pointermove", (e) => {
      if (disc.hasPointerCapture(e.pointerId)) at(e);
    });
    disc.addEventListener("pointerup", (e) => {
      if (disc.hasPointerCapture(e.pointerId)) {
        disc.releasePointerCapture(e.pointerId);
        this.done();
      }
    });
    // Teclado: ←/→ gira o matiz, ↑/↓ muda a saturação.
    disc.addEventListener("keydown", (e) => {
      const step = e.shiftKey ? 15 : 5;
      if (e.key === "ArrowLeft") this.hsv.h = (this.hsv.h - step + 360) % 360;
      else if (e.key === "ArrowRight") this.hsv.h = (this.hsv.h + step) % 360;
      else if (e.key === "ArrowUp") this.hsv.s = Math.min(1, this.hsv.s + step / 100);
      else if (e.key === "ArrowDown") this.hsv.s = Math.max(0, this.hsv.s - step / 100);
      else return;
      e.preventDefault();
      this.update(true);
      window.clearTimeout(this.commitTimer);
      this.commitTimer = window.setTimeout(() => this.done(), 500);
    });
  }

  private done() {
    window.clearTimeout(this.commitTimer);
    this.preview(null);
    this.commit(hsvToHex(this.hsv));
  }

  private update(live: boolean) {
    const { h, s, v } = this.hsv;
    const a = (h * Math.PI) / 180;
    this.dot.style.left = `${50 + Math.cos(a) * s * 50}%`;
    this.dot.style.top = `${50 + Math.sin(a) * s * 50}%`;
    const hex = hsvToHex(this.hsv);
    this.dot.style.background = hex;
    this.bright.value = String(Math.round(v * 100));
    this.bright.style.setProperty("--pick", hsvToHex({ h, s, v: 1 }));
    this.canvas.style.filter = `brightness(${0.55 + v * 0.45})`;
    if (live) {
      this.el.removeAttribute("data-idle");
      this.preview(hex);
    }
  }

  /** Matiz em volta, saturação do centro (branco) para a borda. */
  private drawDisc() {
    const n = this.canvas.width;
    const ctx = this.canvas.getContext("2d");
    if (!ctx) return;
    const img = ctx.createImageData(n, n);
    const c = (n - 1) / 2;
    for (let y = 0; y < n; y++) {
      for (let x = 0; x < n; x++) {
        const dx = x - c;
        const dy = y - c;
        const r = Math.hypot(dx, dy) / c;
        const i = (y * n + x) * 4;
        if (r > 1) continue; // fora do círculo: transparente
        const hex = hsvToHex({ h: ((Math.atan2(dy, dx) * 180) / Math.PI + 360) % 360, s: r, v: 1 });
        img.data[i] = parseInt(hex.slice(1, 3), 16);
        img.data[i + 1] = parseInt(hex.slice(3, 5), 16);
        img.data[i + 2] = parseInt(hex.slice(5, 7), 16);
        img.data[i + 3] = r > 0.985 ? Math.round((1 - r) / 0.015 * 255) : 255; // borda suave
      }
    }
    ctx.putImageData(img, 0, 0);
  }
}
