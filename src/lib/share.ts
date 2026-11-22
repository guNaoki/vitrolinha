// Cartão para compartilhar: a vitrola (ou a estante) desenhada num canvas, salva como PNG em
// Imagens\Vitrolinha e copiada para a área de transferência. Tudo local; nada é enviado.
import { invoke } from "@tauri-apps/api/core";
import type { ShelfRecord } from "./ipc";
import { t } from "./i18n";
import type { NowPlaying } from "./state";

const W = 1080;
const H = 1350; // 4:5, bom em redes sociais
const CREAM = "#efe2c3";
const ARTIST = "#d8cba9";
const DIM = "#b8a888";
const BG = "#0d0a07";
const SPOT = "#2a1c0d";
const AMBER = "#e8a33d";
const SERIF = '"IM Fell English", Georgia, serif';
const CAPS = '"Krona One", "Segoe UI", sans-serif';
const REPO = "github.com/guNaoki/vitrolinha";

export type ShareResult = { path: string | null; copied: boolean };

// ---------- Utilidades ----------

/** Capa via Rust (arquivo do app): evita canvas "contaminado" por imagem de outra origem. */
async function loadCover(path: string | null): Promise<ImageBitmap | null> {
  if (!path) return null;
  try {
    const bytes = await invoke<ArrayBuffer>("read_cover", { path });
    return await createImageBitmap(new Blob([bytes]));
  } catch {
    return null;
  }
}

const rgb = (hex: string): [number, number, number] => {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex);
  const n = parseInt(m ? m[1] : AMBER.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
};
const rgba = (hex: string, a: number, k = 1) => {
  const [r, g, b] = rgb(hex);
  return `rgba(${Math.round(r * k)},${Math.round(g * k)},${Math.round(b * k)},${a})`;
};

/** Desenha a imagem preenchendo o quadrado (recorte central, como `background-size: cover`). */
function drawCover(ctx: CanvasRenderingContext2D, img: ImageBitmap, x: number, y: number, size: number) {
  const s = Math.min(img.width, img.height);
  ctx.drawImage(img, (img.width - s) / 2, (img.height - s) / 2, s, s, x, y, size, size);
}

/** Quebra em até `maxLines` linhas, com reticências se não couber. */
function wrap(ctx: CanvasRenderingContext2D, text: string, maxW: number, maxLines: number): string[] {
  const words = text.split(/\s+/).filter(Boolean);
  const lines: string[] = [];
  let line = "";
  for (let i = 0; i < words.length; i++) {
    const test = line ? `${line} ${words[i]}` : words[i];
    if (ctx.measureText(test).width <= maxW || !line) {
      line = test;
    } else {
      lines.push(line);
      line = words[i];
      if (lines.length === maxLines - 1) {
        line = words.slice(i).join(" ");
        break;
      }
    }
  }
  if (line) lines.push(line);
  return lines.map((l, i) => (i === maxLines - 1 ? ellipsize(ctx, l, maxW) : l));
}

function ellipsize(ctx: CanvasRenderingContext2D, text: string, maxW: number): string {
  if (ctx.measureText(text).width <= maxW) return text;
  let t = text;
  while (t.length > 1 && ctx.measureText(`${t}…`).width > maxW) t = t.slice(0, -1);
  return `${t.trimEnd()}…`;
}

function background(ctx: CanvasRenderingContext2D, accent: string) {
  ctx.fillStyle = BG;
  ctx.fillRect(0, 0, W, H);
  const spot = ctx.createRadialGradient(W * 0.3, H * 0.3, 0, W * 0.3, H * 0.3, W * 1.05);
  spot.addColorStop(0, SPOT);
  spot.addColorStop(1, BG);
  ctx.fillStyle = spot;
  ctx.fillRect(0, 0, W, H);
  const glow = ctx.createRadialGradient(W / 2, 545, 60, W / 2, 545, 620);
  glow.addColorStop(0, rgba(accent, 0.2));
  glow.addColorStop(1, rgba(accent, 0));
  ctx.fillStyle = glow;
  ctx.fillRect(0, 0, W, H);
}

function vignette(ctx: CanvasRenderingContext2D) {
  const v = ctx.createRadialGradient(W / 2, H / 2, H * 0.35, W / 2, H / 2, H * 0.85);
  v.addColorStop(0, "rgba(0,0,0,0)");
  v.addColorStop(1, "rgba(0,0,0,0.55)");
  ctx.fillStyle = v;
  ctx.fillRect(0, 0, W, H);
}

function footer(ctx: CanvasRenderingContext2D) {
  ctx.textBaseline = "alphabetic";
  ctx.fillStyle = DIM;
  ctx.font = `20px ${CAPS}`;
  ctx.letterSpacing = "0.3em";
  ctx.textAlign = "left";
  ctx.fillText("VITROLINHA", 80, H - 56);
  ctx.letterSpacing = "0.06em";
  ctx.textAlign = "right";
  ctx.fillText(REPO, W - 80, H - 56);
  ctx.letterSpacing = "0px";
  ctx.textAlign = "left";
}

function record(
  ctx: CanvasRenderingContext2D,
  cx: number,
  cy: number,
  R: number,
  cover: ImageBitmap | null,
  accent: string,
) {
  // sombra e disco
  ctx.save();
  ctx.shadowColor = "rgba(0,0,0,0.7)";
  ctx.shadowBlur = 70;
  ctx.shadowOffsetY = 28;
  ctx.fillStyle = "#14110d";
  ctx.beginPath();
  ctx.arc(cx, cy, R, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
  // sulcos
  ctx.lineWidth = 1;
  for (let r = R * 0.45; r < R - 6; r += 5) {
    ctx.strokeStyle = `rgba(239,226,195,${0.035 + 0.03 * ((r * 7) % 3 === 0 ? 1 : 0)})`;
    ctx.beginPath();
    ctx.arc(cx, cy, r, 0, Math.PI * 2);
    ctx.stroke();
  }
  for (const k of [0.6, 0.78, 0.9]) {
    ctx.strokeStyle = "rgba(239,226,195,0.1)";
    ctx.beginPath();
    ctx.arc(cx, cy, R * k, 0, Math.PI * 2);
    ctx.stroke();
  }
  // brilho (dois reflexos opostos)
  const sheen = ctx.createConicGradient(-0.5, cx, cy);
  for (const [at, a] of [[0, 0], [0.06, 0.1], [0.2, 0], [0.5, 0], [0.56, 0.1], [0.7, 0], [1, 0]] as const) {
    sheen.addColorStop(at, `rgba(255,244,214,${a})`);
  }
  ctx.fillStyle = sheen;
  ctx.beginPath();
  ctx.arc(cx, cy, R - 4, 0, Math.PI * 2);
  ctx.fill();
  // selo (capa)
  const L = R * 0.42;
  ctx.fillStyle = rgba(accent, 1, 0.87);
  ctx.beginPath();
  ctx.arc(cx, cy, L, 0, Math.PI * 2);
  ctx.fill();
  if (cover) {
    ctx.save();
    ctx.beginPath();
    ctx.arc(cx, cy, L, 0, Math.PI * 2);
    ctx.clip();
    drawCover(ctx, cover, cx - L, cy - L, L * 2);
    ctx.restore();
  }
  ctx.strokeStyle = rgba(accent, 1, 0.55);
  ctx.lineWidth = 6;
  ctx.beginPath();
  ctx.arc(cx, cy, L, 0, Math.PI * 2);
  ctx.stroke();
  // furo
  ctx.fillStyle = BG;
  ctx.beginPath();
  ctx.arc(cx, cy, 14, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = "rgba(239,226,195,0.35)";
  ctx.lineWidth = 2;
  ctx.stroke();
}

/** Pixels do canvas no formato DIB (24 bits, de baixo para cima) que a área de transferência aceita. */
function toDib(ctx: CanvasRenderingContext2D): Uint8Array {
  const { data } = ctx.getImageData(0, 0, W, H);
  const out = new Uint8Array(40 + W * H * 3); // W*3 é múltiplo de 4: sem preenchimento de linha
  const head = new DataView(out.buffer);
  head.setUint32(0, 40, true); // tamanho do cabeçalho
  head.setInt32(4, W, true);
  head.setInt32(8, H, true); // positivo = de baixo para cima
  head.setUint16(12, 1, true); // planos
  head.setUint16(14, 24, true); // bits por pixel
  head.setUint32(20, W * H * 3, true);
  let o = 40;
  for (let y = H - 1; y >= 0; y--) {
    for (let x = 0; x < W; x++) {
      const i = (y * W + x) * 4;
      out[o++] = data[i + 2];
      out[o++] = data[i + 1];
      out[o++] = data[i];
    }
  }
  return out;
}

async function finish(canvas: HTMLCanvasElement, ctx: CanvasRenderingContext2D, name: string): Promise<ShareResult> {
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
  if (!blob) throw new Error("não foi possível gerar a imagem");
  let copied = false;
  try {
    await invoke("copy_share_image", toDib(ctx));
    copied = true;
  } catch (e) {
    console.warn("área de transferência (Windows):", e);
    try {
      await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
      copied = true;
    } catch {
      /* sem permissão: ainda salva o arquivo */
    }
  }
  let path: string | null = null;
  try {
    path = await invoke<string>("save_share_image", new Uint8Array(await blob.arrayBuffer()), {
      headers: { "x-name": encodeURIComponent(name) },
    });
  } catch (e) {
    console.warn("não foi possível salvar o cartão:", e);
  }
  return { path, copied };
}

async function newCanvas(): Promise<{ canvas: HTMLCanvasElement; ctx: CanvasRenderingContext2D }> {
  // As fontes do tema só existem no canvas depois de carregadas.
  await Promise.all([
    document.fonts.load(`64px ${SERIF}`),
    document.fonts.load(`italic 40px ${SERIF}`),
    document.fonts.load(`24px ${CAPS}`),
  ]).catch(() => {});
  const canvas = document.createElement("canvas");
  canvas.width = W;
  canvas.height = H;
  return { canvas, ctx: canvas.getContext("2d")! };
}

// ---------- Cartões ----------

/** O disco que está tocando agora, com título e artista. */
export async function shareNowPlaying(s: NowPlaying): Promise<ShareResult> {
  const { canvas, ctx } = await newCanvas();
  const accent = s.accentColor ?? AMBER;
  const cover = await loadCover(s.artworkPath);
  background(ctx, accent);
  record(ctx, W / 2, 545, 410, cover, accent);
  vignette(ctx);

  ctx.textBaseline = "alphabetic";
  ctx.fillStyle = CREAM;
  ctx.font = `66px ${SERIF}`;
  const title = wrap(ctx, s.title || "—", W - 160, 2);
  // Título de duas linhas sobe um pouco, para o artista não encostar no rodapé.
  const y0 = 1135 - (title.length - 1) * 42;
  ctx.fillStyle = accent;
  ctx.font = `22px ${CAPS}`;
  ctx.letterSpacing = "0.28em";
  ctx.fillText(t(s.status === "playing" ? "TOCANDO AGORA" : "NA VITROLA"), 80, y0 - 70);
  ctx.letterSpacing = "0px";

  ctx.fillStyle = CREAM;
  ctx.font = `66px ${SERIF}`;
  title.forEach((l, i) => ctx.fillText(l, 80, y0 + i * 74));
  let y = y0 + (title.length - 1) * 74 + 62;
  if (s.artist) {
    ctx.fillStyle = ARTIST;
    ctx.font = `italic 40px ${SERIF}`;
    ctx.fillText(ellipsize(ctx, s.artist, W - 160), 80, y);
    y += 44;
  }
  if (s.album && y < H - 90) {
    ctx.fillStyle = DIM;
    ctx.font = `28px ${SERIF}`;
    ctx.fillText(ellipsize(ctx, s.album, W - 160), 80, y);
  }
  footer(ctx);
  return finish(canvas, ctx, [s.artist, s.title].filter(Boolean).join(" - "));
}

/** Os discos mais ouvidos da estante (até 9), com quantas vezes cada um tocou. */
export async function shareShelf(records: ShelfRecord[]): Promise<ShareResult> {
  const top = [...records].sort((a, b) => b.plays - a.plays || b.lastMs - a.lastMs).slice(0, 9);
  const { canvas, ctx } = await newCanvas();
  const covers = await Promise.all(top.map((r) => loadCover(r.coverPath)));
  background(ctx, AMBER);

  ctx.textBaseline = "alphabetic";
  ctx.fillStyle = AMBER;
  ctx.font = `22px ${CAPS}`;
  ctx.letterSpacing = "0.28em";
  ctx.fillText(t("MINHA ESTANTE"), 80, 130);
  ctx.letterSpacing = "0px";
  ctx.fillStyle = CREAM;
  ctx.font = `92px ${SERIF}`;
  ctx.fillText(t("Os mais ouvidos"), 80, 235);

  const cell = 280;
  const gap = 40;
  top.forEach((r, i) => {
    const col = i % 3;
    const row = Math.floor(i / 3);
    const x = 80 + col * (cell + gap);
    const y = 300 + row * (cell + 118);
    ctx.save();
    ctx.shadowColor = "rgba(0,0,0,0.65)";
    ctx.shadowBlur = 28;
    ctx.shadowOffsetY = 12;
    ctx.fillStyle = "#2a251e";
    ctx.fillRect(x, y, cell, cell);
    ctx.restore();
    const img = covers[i];
    if (img) drawCover(ctx, img, x, y, cell);
    else {
      ctx.fillStyle = "#2a251e";
      ctx.fillRect(x, y, cell, cell);
      ctx.fillStyle = DIM;
      ctx.font = `96px ${SERIF}`;
      ctx.textAlign = "center";
      ctx.fillText("♪", x + cell / 2, y + cell / 2 + 32);
      ctx.textAlign = "left";
    }
    // contagem de vezes
    if (r.plays > 1) {
      const tag = `×${r.plays}`;
      ctx.font = `18px ${CAPS}`;
      const w = ctx.measureText(tag).width + 22;
      ctx.fillStyle = "rgba(13,10,7,0.82)";
      ctx.fillRect(x + cell - w, y, w, 36);
      ctx.fillStyle = AMBER;
      ctx.fillText(tag, x + cell - w + 11, y + 25);
    }
    ctx.fillStyle = CREAM;
    ctx.font = `26px ${SERIF}`;
    ctx.fillText(ellipsize(ctx, r.title, cell), x, y + cell + 34);
    ctx.fillStyle = DIM;
    ctx.font = `italic 22px ${SERIF}`;
    ctx.fillText(ellipsize(ctx, r.artist, cell), x, y + cell + 64);
  });
  if (top.length === 0) {
    ctx.fillStyle = DIM;
    ctx.font = `italic 36px ${SERIF}`;
    ctx.fillText(t("Ainda não há discos na estante."), 80, 400);
  }
  vignette(ctx);
  footer(ctx);
  return finish(canvas, ctx, "estante");
}
