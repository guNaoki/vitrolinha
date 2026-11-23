// Atmosfera do painel: o brilho da cor no fundo (--glow) e o granulado de filme (--grain).
import { getLook, onLook, type Look } from "./ipc";

/** Aplica os valores (0–100) como variáveis de CSS. */
export function paintLook(l: Look) {
  const root = document.documentElement.style;
  root.setProperty("--glow", String(l.glow / 100));
  root.setProperty("--grain", String((l.grain / 100) * 0.6)); // opacidade do grão: 0 a 0,6
}

export function initLook() {
  void getLook().then(paintLook).catch(() => {});
  void onLook(paintLook);
}
