// Estado único do front-end. O tipo espelha `NowPlaying` em src-tauri/src/media.rs.

export type PlaybackStatus = "playing" | "paused" | "stopped";

export type NowPlaying = {
  title: string;
  artist: string;
  album: string;
  status: PlaybackStatus;
  positionMs: number; // posição no momento da amostra
  durationMs: number; // 0 se a fonte não informa
  sampledAtMs: number; // relógio do sistema (epoch, ms) quando positionMs foi lido
  artworkPath: string | null; // JPEG temporário ~512px
  accentColor: string | null; // "#rrggbb" da capa; null = âmbar padrão (ou recurso desligado)
  sourceApp: string;
};

/** Posição estimada agora, interpolada a partir da amostra. */
export function livePositionMs(s: NowPlaying, nowMs = Date.now()): number {
  const elapsed = s.status === "playing" ? Math.max(0, nowMs - s.sampledAtMs) : 0;
  const pos = s.positionMs + elapsed;
  return s.durationMs > 0 ? Math.min(pos, s.durationMs) : pos;
}

export function formatTime(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(total / 60);
  const sec = String(total % 60).padStart(2, "0");
  return `${m}:${sec}`;
}
