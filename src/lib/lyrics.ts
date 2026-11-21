// Letra sincronizada: tipos (espelham lyrics.rs) e a view que acompanha a posição.
//
// O LRC só dá o instante de cada LINHA. O brilho palavra por palavra é estimado:
// o tempo da linha é repartido entre as palavras na proporção do tamanho delas.
// A animação usa só `transform` e `opacity`, com `transition-delay` por palavra
// (nada de timers por palavra): o compositor faz o resto.

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

/** Palavras da linha. */
export function splitWords(text: string): string[] {
  return text.split(/\s+/).filter(Boolean);
}

/**
 * Quanto a linha dura (ms): até a próxima, mas sem esticar por intervalos longos
 * (instrumentais): ~0,5 s por palavra + folga. Mínimo de 0,8 s.
 */
export function lineDurationMs(lines: LyricLine[], idx: number): number {
  const cur = lines[idx];
  const next = lines[idx + 1];
  const words = splitWords(cur.text).length;
  const gap = next ? next.timeMs - cur.timeMs : 5000;
  return Math.max(800, Math.min(gap, 600 + words * 520));
}

/**
 * Instante (ms desde o início da linha) em que cada palavra acende. A última
 * acende antes do fim da linha (a linha ocupa 90% do tempo), proporcional ao
 * tamanho das palavras.
 */
export function wordStarts(words: string[], durationMs: number): number[] {
  const weight = (w: string) => Math.max(2, w.length) + 1;
  const total = words.reduce((n, w) => n + weight(w), 0);
  const usable = durationMs * 0.9;
  let acc = 0;
  return words.map((w) => {
    const start = (acc / total) * usable;
    acc += weight(w);
    return Math.round(start);
  });
}

/** Linhas mostradas, relativas à atual: uma antes, a atual e duas depois. */
const OFFSETS = [-1, 0, 1, 2] as const;
const CURRENT = 1; // posição da linha atual em OFFSETS

/** Adianta a letra um pouco: a posição do player chega com atraso, e a letra pede para ser vista antes de ouvida. */
const LEAD_MS = 160;
/** Reagendar o brilho só se o ponto da linha mudou mais que isso (evita refazer a cada atualização do player). */
const DRIFT_MS = 250;

const reducedMotion =() => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * Mostra a letra e a mantém sincronizada. Sem polling: um único setTimeout
 * agendado para a próxima linha, só enquanto toca e a janela está visível.
 */
export class LyricsView {
  private lines: LyricLine[] = [];
  private timer: number | undefined;
  private shownIndex = Number.NaN;
  private wordEls: HTMLElement[] = [];
  private starts: number[] = [];
  /** Quando (performance.now) a linha atual começou, e se estava tocando, na última vez que o brilho foi agendado. */
  private anchor = Number.NaN;
  private wasRunning = false;

  constructor(
    private readonly root: HTMLElement,
    private readonly slots: HTMLElement[],
  ) {}

  /** Troca a letra (null = sem letra). */
  setLyrics(lines: LyricLine[] | null) {
    this.lines = lines ?? [];
    this.shownIndex = Number.NaN;
    this.wordEls = [];
    this.anchor = Number.NaN;
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
    const changed = idx !== this.shownIndex;
    if (changed) this.renderLines(idx);
    this.applyTiming(idx >= 0 ? pos - this.lines[idx].timeMs : 0, running, changed);

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
      slot.replaceChildren();
      if (i === CURRENT) {
        this.buildCurrent(slot, idx);
      } else {
        slot.textContent = line?.text ?? "";
      }
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

  /** Linha atual: uma <span> por palavra (a camada acesa é um ::after, ver CSS). */
  private buildCurrent(slot: HTMLElement, idx: number) {
    this.wordEls = [];
    const line = this.lines[idx];
    if (!line) return; // antes da primeira linha
    const words = splitWords(line.text);
    if (words.length === 0) {
      slot.textContent = "♪"; // pausa instrumental
      return;
    }
    this.starts = wordStarts(words, lineDurationMs(this.lines, idx));
    words.forEach((w, i) => {
      if (i > 0) slot.append(" ");
      const span = document.createElement("span");
      span.className = "w";
      span.dataset.t = w;
      span.textContent = w;
      slot.append(span);
      this.wordEls.push(span);
    });
  }

  /**
   * Acende as palavras conforme o ponto da linha em que estamos.
   * Tocando: cada palavra recebe um `transition-delay` (negativo se já passou,
   * o que a deixa acesa na hora). Pausado: acesas as que já passaram, sem animar.
   */
  private applyTiming(elapsedMs: number, running: boolean, fresh: boolean) {
    const slot = this.slots[CURRENT];
    slot.classList.toggle("static", !running);
    if (this.wordEls.length === 0) return;
    // O player manda atualizações de posição o tempo todo: se o brilho já agendado
    // continua certo, não mexe (refazer apaga e reacende as palavras → pisca/atrasa).
    const anchor = performance.now() - elapsedMs;
    if (!fresh && running === this.wasRunning && Math.abs(anchor - this.anchor) < DRIFT_MS) return;
    this.anchor = anchor;
    this.wasRunning = running;
    if (!fresh) {
      this.wordEls.forEach((w) => w.classList.remove("on"));
      void slot.offsetWidth; // fecha o estado "apagado" antes de reacender
    }
    this.wordEls.forEach((w, i) => {
      const delay = this.starts[i] - elapsedMs;
      w.style.setProperty("--d", `${delay}ms`);
      if (running || delay <= 0) w.classList.add("on");
    });
  }
}
