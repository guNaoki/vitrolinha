// invoke/listen tipados — único ponto de contato com o Rust.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Lyrics } from "./lyrics";
import type { NowPlaying } from "./state";

export const getState = () => invoke<NowPlaying | null>("get_state");

export const getLyrics = () => invoke<Lyrics | null>("get_lyrics");

/** Letra da faixa atual (null = limpar). Chega depois do now-playing da faixa. */
export const onLyrics = (cb: (l: Lyrics | null) => void): Promise<UnlistenFn> =>
  listen<Lyrics | null>("lyrics", (e) => cb(e.payload));

export const togglePlayPause = () => invoke<void>("toggle_play_pause");
export const next = () => invoke<void>("next");
export const previous = () => invoke<void>("previous");

/** Minimiza o painel (volta pela barra de tarefas ou pelo ícone da bandeja). */
export const minimizePanel = () => invoke<void>("minimize_panel");

/** Alterna entre ocupar o monitor inteiro e a janela livre (menor, redimensionável). */
export const togglePanelSize = () => invoke<void>("toggle_panel_size");

export const onNowPlaying =(cb: (s: NowPlaying) => void): Promise<UnlistenFn> =>
  listen<NowPlaying>("now-playing", (e) => cb(e.payload));

/** Popup: `true` logo após aparecer (toca a entrada), `false` logo antes de esconder. */
export const onPopupVisible = (cb: (visible: boolean) => void): Promise<UnlistenFn> =>
  listen<boolean>("popup-visible", (e) => cb(e.payload));

/** Popup: começar a animação de saída (a janela some um instante depois). */
export const onPopupLeave = (cb: () => void): Promise<UnlistenFn> =>
  listen("popup-leave", () => cb());

/** Estante: um disco ouvido (espelha `Record` em history.rs). */
export type ShelfRecord = {
  title: string;
  artist: string;
  album: string;
  coverPath: string | null;
  plays: number;
  lastMs: number;
};
export const getHistory = () => invoke<ShelfRecord[]>("get_history");

/** Volume do sistema (0–1). `null` se não há dispositivo de saída. */
export type Volume = { level: number; muted: boolean };
export const getVolume = () => invoke<Volume | null>("get_volume");
export const setVolume = (level: number) => invoke<Volume | null>("set_volume", { level });
export const toggleMute = () => invoke<Volume | null>("toggle_mute");

/** Efeitos do disco: "off" | "soft" | "strong" (menu da bandeja). */
export type DiskFx = "off" | "soft" | "strong";
export const getDiskFx = () => invoke<DiskFx>("get_disk_fx");
export const onDiskFx = (cb: (level: DiskFx) => void): Promise<UnlistenFn> =>
  listen<DiskFx>("disk-fx", (e) => cb(e.payload));

/** Modo de janela livre (arrastar/redimensionar) ligado? */
export const isPanelFree = () => invoke<boolean>("panel_is_free");
export const onPanelFree = (cb: (free: boolean) => void): Promise<UnlistenFn> =>
  listen<boolean>("panel-free", (e) => cb(e.payload));

/** O Rust avisa antes de esconder e depois de mostrar a janela do painel. */
export const onPanelVisible = (cb: (visible: boolean) => void): Promise<UnlistenFn> =>
  listen<boolean>("panel-visible", (e) => cb(e.payload));
