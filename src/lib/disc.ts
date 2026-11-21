// Disco: capa no selo. O giro é puro CSS e depende só de body[data-status].
import { convertFileSrc } from "@tauri-apps/api/core";

let shownPath: string | null | undefined;

export function setCover(el: HTMLElement, path: string | null) {
  if (path === shownPath) return;
  shownPath = path;
  el.style.backgroundImage = path ? `url("${convertFileSrc(path)}")` : "";
}
