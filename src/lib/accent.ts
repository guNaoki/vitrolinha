// Cor de destaque: sobrescreve --amber (a transição vem do @property em tokens.css).
// Vale, em ordem: a cor escolhida à mão, a da capa (vem no estado da faixa), o âmbar padrão.
import { getAccent, onAccent } from "./ipc";

const HEX = /^#[0-9a-f]{6}$/i;
let custom: string | null = null; // escolhida à mão
let fromTrack: string | null = null; // da capa

function paint(color: string | null | undefined) {
  const root = document.documentElement.style;
  if (color && HEX.test(color)) {
    root.setProperty("--amber", color);
  } else {
    root.removeProperty("--amber"); // volta ao âmbar padrão
  }
}

/** Chamado a cada estado da faixa. */
export function applyAccent(color: string | null | undefined) {
  fromTrack = color ?? null;
  paint(custom ?? fromTrack);
}

/** Lê a cor escolhida e acompanha mudanças (vale também sem nada tocando). */
export function initAccent() {
  void getAccent()
    .then((c) => {
      custom = c;
      paint(custom ?? fromTrack);
    })
    .catch(() => {});
  void onAccent((c) => {
    custom = c;
    paint(custom ?? fromTrack);
  });
}

/** Prévia ao vivo (arrastando o seletor), sem gravar. `null` desfaz a prévia. */
export function previewAccent(color: string | null) {
  paint(color ?? custom ?? fromTrack);
}
