// "Vinil": velocidade do giro (16/33/45 rpm, vale no painel e no popup) e o som de disco antigo.
import { getVinyl, onVinyl, type Vinyl } from "./ipc";

/** Uma volta do disco, em segundos (33⅓ rpm = 1,8 s). */
const SPIN_S: Record<number, number> = { 16: 3.6, 33: 1.8, 45: 60 / 45 };
const spinSeconds = (rpm: number) => SPIN_S[rpm] ?? 1.8;

export type CrackleLevel = Vinyl["crackle"];

/** Lê o que está salvo e acompanha mudanças do menu. `onChange` recebe o estado todo. */
export function initVinyl(onChange?: (v: Vinyl) => void) {
  const apply = (v: Vinyl) => {
    document.documentElement.style.setProperty("--spin", `${spinSeconds(v.rpm)}s`);
    onChange?.(v);
  };
  void getVinyl().then(apply).catch(() => {});
  void onVinyl(apply);
}

/**
 * Mistura de cada nível. O som do disco antigo tem quatro camadas, como um disco de verdade:
 *  - chiado de superfície: um "shhh" fino e constante (`hiss`);
 *  - estalinhos miúdos de poeira (`ticks`/s) e pipocos maiores (`pops`/s);
 *  - rajadas de arranhão, vários estalinhos colados (`bursts`/s);
 *  - o "tec" de uma risca, que se repete a cada volta do disco (`rev`), no compasso da rotação.
 */
type Mix = { hiss: number; ticks: number; tickAmp: number; pops: number; popAmp: number; bursts: number; rev: number };
const MIX: Record<Exclude<CrackleLevel, "off">, Mix> = {
  soft: { hiss: 0.006, ticks: 3, tickAmp: 0.02, pops: 0.2, popAmp: 0.07, bursts: 0, rev: 0.07 },
  medium: { hiss: 0.01, ticks: 6, tickAmp: 0.035, pops: 0.7, popAmp: 0.12, bursts: 0.15, rev: 0.12 },
  strong: { hiss: 0.016, ticks: 11, tickAmp: 0.05, pops: 1.3, popAmp: 0.2, bursts: 0.4, rev: 0.2 },
};
const LOOP_TARGET_S = 12; // trecho gerado uma vez e repetido (múltiplo exato de voltas)
const FADE_S = 0.6;

/**
 * Som de disco antigo gerado na hora (sem arquivo de áudio) por cima do que você ouve. Só soa
 * tocando e com o painel visível; ao pausar, esmaece e suspende o áudio (nada roda em segundo
 * plano). Não dá para "envelhecer" o som do player em si (o Windows não deixa mexer no áudio de
 * outro app): isto é o disco rodando por baixo.
 */
export class Crackle {
  private ctx: AudioContext | null = null;
  private out: GainNode | null = null;
  private src: AudioBufferSourceNode | null = null;
  private built = ""; // nível + rotação do trecho que está tocando
  private suspendTimer: number | undefined;

  sync(level: CrackleLevel, audible: boolean, rpm: number) {
    const on = audible && level !== "off";
    window.clearTimeout(this.suspendTimer);
    if (!on) {
      if (this.ctx && this.out) {
        this.fadeTo(0);
        // Depois do esmaecer, suspende o contexto (zero CPU parado).
        this.suspendTimer = window.setTimeout(() => void this.ctx?.suspend(), FADE_S * 1000 + 100);
      }
      return;
    }
    const ctx = this.ensure();
    if (!ctx) return;
    this.load(ctx, level as Exclude<CrackleLevel, "off">, rpm);
    void ctx.resume().catch(() => {});
    this.fadeTo(1);
  }

  /** Troca o trecho quando o nível ou a rotação mudam (as voltas têm que casar com o disco). */
  private load(ctx: AudioContext, level: Exclude<CrackleLevel, "off">, rpm: number) {
    const key = `${level}@${rpm}`;
    if (key === this.built || !this.out) return;
    this.built = key;
    const next = ctx.createBufferSource();
    next.buffer = makeRecordNoise(ctx, MIX[level], spinSeconds(rpm));
    next.loop = true;
    next.connect(this.out);
    next.start();
    const old = this.src;
    this.src = next;
    old?.stop();
    old?.disconnect();
  }

  private fadeTo(value: number) {
    if (!this.ctx || !this.out) return;
    const t = this.ctx.currentTime;
    this.out.gain.cancelScheduledValues(t);
    this.out.gain.setValueAtTime(this.out.gain.value, t);
    this.out.gain.linearRampToValueAtTime(value, t + FADE_S);
  }

  private ensure(): AudioContext | null {
    if (this.ctx) return this.ctx;
    try {
      const ctx = new AudioContext({ latencyHint: "playback" });
      this.out = ctx.createGain();
      this.out.gain.value = 0;
      this.out.connect(ctx.destination);
      this.ctx = ctx;
    } catch (e) {
      console.warn("som de disco antigo indisponível:", e);
    }
    return this.ctx;
  }
}

/** Um estalo curto: ruído com decaimento rápido, de sinal aleatório. `tau` em segundos. */
function click(d: Float32Array, rate: number, at: number, amp: number, tau: number) {
  const len = Math.min(d.length - at, Math.floor(rate * tau * 6));
  const sign = Math.random() < 0.5 ? -1 : 1;
  for (let i = 0; i < len; i++) {
    d[at + i] += sign * amp * Math.exp(-i / (rate * tau)) * (Math.random() * 0.7 + 0.3);
  }
}

/** O "tec" de uma risca: estalo agudo seco + uma pancadinha grave logo depois. */
function tec(d: Float32Array, rate: number, at: number, amp: number) {
  const len = Math.min(d.length - at, Math.floor(rate * 0.06));
  for (let i = 0; i < len; i++) {
    const t = i / rate;
    const snap = Math.exp(-t / 0.0007) * (0.6 * (Math.random() * 2 - 1) + 0.8 * Math.sin(2 * Math.PI * 2600 * t));
    const thump = Math.exp(-t / 0.012) * 0.45 * Math.sin(2 * Math.PI * 150 * t);
    d[at + i] += amp * (snap + thump);
  }
}

/**
 * O trecho que se repete: um número inteiro de voltas (para o "tec" cair sempre no mesmo ponto
 * da volta, no compasso do disco girando), sempre perto de 12 s.
 */
export function makeRecordNoise(ctx: AudioContext, mix: Mix, spin: number): AudioBuffer {
  const rate = ctx.sampleRate;
  const turns = Math.max(1, Math.round(LOOP_TARGET_S / spin));
  const seconds = turns * spin;
  const buf = ctx.createBuffer(1, Math.floor(rate * seconds), rate);
  const d = buf.getChannelData(0);

  // Chiado de superfície: ruído branco passado por um filtro de faixa simples (≈ 1–6 kHz).
  let prev = 0;
  let lp = 0;
  for (let i = 0; i < d.length; i++) {
    const white = Math.random() * 2 - 1;
    lp += 0.45 * (white - prev - lp); // passa-altas (diferença) seguido de passa-baixas leve
    prev = white;
    d[i] = lp * mix.hiss;
  }
  // O chiado oscila devagar de volume (a agulha não anda sempre igual).
  const sway = Math.random() * 6.28;
  for (let i = 0; i < d.length; i++) {
    d[i] *= 0.8 + 0.2 * Math.sin(sway + (i / rate) * 0.9);
  }

  const poisson = (perSecond: number, each: (at: number) => void) => {
    const n = Math.round(perSecond * seconds);
    for (let k = 0; k < n; k++) each(Math.floor(Math.random() * (d.length - rate * 0.1)));
  };
  // Poeira: estalinhos miúdos.
  poisson(mix.ticks, (at) => click(d, rate, at, mix.tickAmp * (0.3 + Math.random() * 0.7), 0.0004 + Math.random() * 0.0006));
  // Pipocos maiores, mais "gordos".
  poisson(mix.pops, (at) => click(d, rate, at, mix.popAmp * (0.5 + Math.random() * 0.5), 0.002 + Math.random() * 0.003));
  // Arranhões: uma rajada de estalinhos em sequência, de 60 a 150 ms.
  poisson(mix.bursts, (at) => {
    const count = 8 + Math.floor(Math.random() * 18);
    const span = rate * (0.06 + Math.random() * 0.09);
    for (let j = 0; j < count; j++) {
      click(d, rate, Math.min(d.length - 1, at + Math.floor(Math.random() * span)), mix.tickAmp * (0.6 + Math.random() * 0.8), 0.0005);
    }
  });
  // O "tec" que volta a cada volta do disco (com uma "irmã" mais fraca, se o nível é alto).
  const where = Math.floor(rate * spin * (0.2 + Math.random() * 0.6)); // ponto da volta onde está a risca
  for (let k = 0; k < turns; k++) {
    tec(d, rate, Math.floor(k * rate * spin) + where, mix.rev);
    if (mix.rev > 0.1) tec(d, rate, Math.floor(k * rate * spin) + Math.floor(where * 0.37), mix.rev * 0.45);
  }

  // Limita o pico (sem estourar) e emenda o fim com o começo, sem clique.
  let peak = 0;
  for (let i = 0; i < d.length; i++) peak = Math.max(peak, Math.abs(d[i]));
  if (peak > 0.6) for (let i = 0; i < d.length; i++) d[i] *= 0.6 / peak;
  const edge = Math.floor(rate * 0.02);
  for (let i = 0; i < edge; i++) {
    const k = i / edge;
    d[i] *= k;
    d[d.length - 1 - i] *= k;
  }
  return buf;
}
