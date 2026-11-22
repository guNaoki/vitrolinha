// Só em `npm run dev` no navegador (sem o Rust): finge os comandos do Tauri para dar para ver e
// testar o painel. Em produção `import.meta.env.DEV` é falso e este arquivo vira um módulo vazio.
//
// Console: `__mock.set({ canSeek: false })`, `__mock.set({ durationMs: 0 })`, `__mock.set({ status: "paused" })`.
import type { NowPlaying } from "./state";

type Internals = {
  invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
  transformCallback: (cb: (e: unknown) => void, once?: boolean) => number;
  convertFileSrc: (path: string) => string;
};

function install() {
  const w = window as unknown as { __TAURI_INTERNALS__?: Internals; __mock?: unknown };
  if (w.__TAURI_INTERNALS__) return; // dentro do app de verdade

  let now: NowPlaying = {
    title: "Tarde em Itapoã",
    artist: "Vinicius de Moraes",
    album: "Toquinho e Vinicius",
    status: "playing",
    positionMs: 72_000,
    durationMs: 225_000,
    sampledAtMs: Date.now(),
    canSeek: true,
    artworkPath: null,
    accentColor: null,
    sourceApp: "Spotify.exe",
  };
  let volume = { level: 0.4, muted: false };

  const callbacks = new Map<number, (e: unknown) => void>();
  const listeners = new Map<string, number[]>();
  let nextId = 1;
  const emit = (event: string, payload: unknown) =>
    (listeners.get(event) ?? []).forEach((id) => callbacks.get(id)?.({ event, id, payload }));

  const commands: Record<string, (a: Record<string, unknown>) => unknown> = {
    get_state: () => now,
    get_lyrics: () => null,
    get_voice: () => ({ status: "off", channel: null, members: [] }),
    get_volume: () => volume,
    set_volume: (a) => (volume = { ...volume, level: Number(a.level) }),
    toggle_mute: () => (volume = { ...volume, muted: !volume.muted }),
    get_disk_fx: () => "soft",
    get_history: () => [],
    panel_is_free: () => false,
    toggle_play_pause: () => {
      const pos = livePos();
      now = { ...now, status: now.status === "playing" ? "paused" : "playing", positionMs: pos, sampledAtMs: Date.now() };
      emit("now-playing", now);
    },
    seek: (a) => {
      now = { ...now, positionMs: Number(a.positionMs), sampledAtMs: Date.now() };
      console.info("[mock] seek →", a.positionMs);
    },
  };
  const livePos = () => now.positionMs + (now.status === "playing" ? Date.now() - now.sampledAtMs : 0);

  w.__TAURI_INTERNALS__ = {
    transformCallback: (cb) => {
      const id = nextId++;
      callbacks.set(id, cb);
      return id;
    },
    convertFileSrc: (p) => p,
    invoke: async (cmd, args = {}) => {
      if (cmd === "plugin:event|listen") {
        const ev = String(args.event);
        listeners.set(ev, [...(listeners.get(ev) ?? []), Number(args.handler)]);
        return nextId++;
      }
      if (cmd.startsWith("plugin:event|")) return null;
      const fn = commands[cmd];
      if (!fn) throw new Error(`[mock] comando sem mock: ${cmd}`);
      return fn(args);
    },
  };
  w.__mock = {
    set: (patch: Partial<NowPlaying>) => {
      now = { ...now, positionMs: livePos(), sampledAtMs: Date.now(), ...patch };
      emit("now-playing", now);
    },
  };
}

if (import.meta.env.DEV) install();
