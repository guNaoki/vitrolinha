// Letra sincronizada: tipos (espelham lyrics.rs) e a view que acompanha a posição.
//
// O LRC só dá o instante de cada LINHA, então a letra acompanha por linha (a atual acesa,
// as vizinhas apagadas). Sem polling: um único setTimeout para a próxima linha.

export type LyricLine = { timeMs: number; text: string };
export type Lyrics = { title: string; artist: string; lines: LyricLine[] };

/** Índice da última linha com timeMs <= posMs; -1 antes da primeira. */
export function currentLineIndex(lines: LyricLine[], posMs: number): number {
  let lo = 0;
  let hi = lines.length - 1;
  let found = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid].timeMs <= posMs) {
      found = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  return found;
}

/** Linhas mostradas, relativas à atual: uma antes, a atual e duas depois. */
const OFFSETS = [-1, 0, 1, 2] as const;
const CURRENT = 1; // posição da linha atual em OFFSETS

/** Adianta a letra um pouco: a posição do player chega com atraso, e a letra pede para ser vista antes de ouvida. */
const LEAD_MS = 160;

const reducedMotion =() => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * Mostra a letra e a mantém sincronizada. Sem polling: um único setTimeout
 * agendado para a próxima linha, só enquanto toca e a janela está visível.
 */
export class LyricsView {
  private lines: LyricLine[] = [];
  private timer: number | undefined;
  private shownIndex = Number.NaN;

  constructor(
    private readonly root: HTMLElement,
    private readonly slots: HTMLElement[],
  ) {}

  /** Troca a letra (null = sem letra). */
  setLyrics(lines: LyricLine[] | null) {
    this.lines = lines ?? [];
    this.shownIndex = Number.NaN;
    this.root.hidden = this.lines.length === 0;
    window.clearTimeout(this.timer);
  }

  hasLyrics(): boolean {
    return this.lines.length > 0;
  }

  /**
   * Redesenha para a posição atual e agenda a próxima troca.
   * `getPos` é consultada de novo a cada agendamento (acompanha seek/pausa).
   */
  sync(getPos: () => number, running: boolean) {
    window.clearTimeout(this.timer);
    if (this.lines.length === 0) return;

    const pos = getPos() + (running ? LEAD_MS : 0);
    const idx = currentLineIndex(this.lines, pos);
    if (idx !== this.shownIndex) this.renderLines(idx);

    if (!running) return;
    const next = this.lines[idx + 1];
    if (next) {
      // +30ms: cai logo depois da marca, nunca antes.
      const delay = Math.max(40, next.timeMs - pos + 30);
      this.timer = window.setTimeout(() => this.sync(getPos, running), delay);
    }
  }

  /** Para tudo (janela escondida). */
  stop() {
    window.clearTimeout(this.timer);
  }

  private renderLines(idx: number) {
    const advanced = Number.isFinite(this.shownIndex) && idx === this.shownIndex + 1;
    this.shownIndex = idx;

    OFFSETS.forEach((off, i) => {
      const slot = this.slots[i];
      const line = this.lines[idx + off];
      // Linha atual vazia (pausa instrumental) mostra ♪; antes da primeira linha, nada.
      slot.textContent = line ? line.text.trim() || (i === CURRENT ? "♪" : "") : "";
    });

    // Troca de linha: tudo sobe com um deslize, a linha nova entra crescendo.
    if (advanced && !reducedMotion()) {
      this.slots.forEach((slot, i) => {
        if (!slot.textContent) return;
        const shift = parseFloat(getComputedStyle(slot).fontSize) * 1.6;
        const grow = i === CURRENT ? " scale(0.94)" : "";
        slot.animate(
          // sem `opacity` no último quadro: termina no valor do CSS (linhas laterais dimmed)
          [{ transform: `translateY(${shift}px)${grow}`, opacity: i === CURRENT ? 0.45 : 0.35 }, { transform: "none" }],
          { duration: 400, easing: "cubic-bezier(0.2, 0.8, 0.2, 1)" },
        );
      });
    }
  }
}
