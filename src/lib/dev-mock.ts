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
    artworkPath: "mock-cover",
    accentColor: null,
    sourceApp: "Spotify.exe",
  };
  let volume = { level: 0.4, muted: false };
  let vinyl = { rpm: 33, crackle: "off" };

  // Menu de opções de mentira (o de verdade vem do Rust, tray.rs): alguns ligar/desligar e escolhas.
  const flags: Record<string, boolean> = { popup: true, panel: true, fill: true, history: true, "dynamic-color": true, lyrics: true, "hd-cover": true, discord: true, "discord-presence": true, overlay: false, shortcuts: true, autostart: false };
  const choices: Record<string, string> = { fx: "fx:soft", rpm: "rpm:33", crackle: "crackle:off", "popup-pos": "popup-pos:top-right", "popup-dur": "popup-dur:4000", source: "source:", monitor: "monitor:0", lang: "lang:auto" };
  const chk = (id: string, label: string) => ({ kind: "check", id, label, checked: flags[id], enabled: true });
  const pick = (group: string, id: string, label: string) => ({ kind: "check", id, label, checked: choices[group] === id, enabled: true });
  const menu = () => [
    { kind: "sub", id: "sources", label: "Fonte de mídia", children: [pick("source", "source:", "Todos os apps"), pick("source", "source:~spotify", "Só Spotify"), pick("source", "source:~music.youtube.com", "Só YouTube Music")] },
    { kind: "sub", id: "popup-menu", label: "Popup", children: [chk("popup", "Mostrar ao trocar de faixa"), { kind: "sub", id: "popup-position", label: "Posição", children: ["Em cima, à esquerda", "Em cima, à direita", "Embaixo, à esquerda", "Embaixo, à direita"].map((p, i) => pick("popup-pos", `popup-pos:${i}`, p)) }] },
    { kind: "sub", id: "panel-menu", label: "Painel", children: [chk("panel", "Mostrar painel"), { kind: "sub", id: "monitors", label: "Monitor", children: ["Monitor 1 — 1920×1080 (principal)", "Monitor 2 — 1080×1920"].map((p, i) => pick("monitor", `monitor:${i}`, p)) }, chk("fill", "Ocupar o monitor inteiro")] },
    { kind: "sub", id: "fx", label: "Efeitos do disco", children: [["off", "Desligados"], ["soft", "Suaves"], ["strong", "Fortes"]].map(([p, l]) => pick("fx", `fx:${p}`, l)) },
    { kind: "sub", id: "vinyl", label: "Vinil", children: [{ kind: "sub", id: "vinyl-rpm", label: "Rotação do disco", children: [16, 33, 45].map((r) => pick("rpm", `rpm:${r}`, `${r} rpm`)) }, { kind: "sub", id: "vinyl-crackle", label: "Som de disco antigo (com o painel aberto)", children: [["off", "Desligado"], ["soft", "Leve"], ["medium", "Médio"], ["strong", "Gasto (muito velho)"]].map(([p, l]) => pick("crackle", `crackle:${p}`, l)) }] },
    { kind: "sub", id: "sleep", label: "Dormir (pausar depois de...)", children: [15, 30, 45, 60, 90].map((m) => ({ kind: "item", id: `sleep:${m}`, label: `${m} minutos`, enabled: true })).concat([{ kind: "item", id: "sleep-cancel", label: "Cancelar o timer", enabled: false }]) },
    chk("dynamic-color", "Cor de destaque da capa"),
    chk("lyrics", "Letras sincronizadas (consulta lrclib.net)"),
    chk("hd-cover", "Capa em alta resolução (consulta iTunes)"),
    chk("discord", "Call do Discord na tela cheia (precisa de configuração)"),
    chk("discord-presence", "Mostrar no Discord o que estou ouvindo"),
    chk("overlay", "Overlay para OBS (http://localhost:7421)"),
    chk("shortcuts", "Atalhos globais (Ctrl+Alt+Espaço, →, ←, ↑)"),
    chk("autostart", "Iniciar com o Windows"),
    { kind: "sub", id: "language", label: "Idioma", children: [["auto", "Automático (idioma do Windows)"], ["pt", "Português"], ["en", "English"]].map(([l, n]) => pick("lang", `lang:${l}`, n)) },
    { kind: "sub", id: "shelf", label: "Estante", children: [chk("history", "Guardar o que eu ouço (só neste computador)"), { kind: "item", id: "clear-history", label: "Limpar a estante (apaga o histórico)", enabled: true }] },
    { kind: "item", id: "logs", label: "Abrir a pasta de logs (para relatar um problema)", enabled: true },
    { kind: "item", id: "quit", label: "Sair", enabled: true },
  ];

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
    get_vinyl: () => vinyl,
    get_menu: () => menu(),
    menu_click: (a) => {
      const id = String(a.id);
      if (id in flags) flags[id] = !flags[id];
      else choices[id.split(":")[0]] = id;
    },
    get_lang: () => (new URLSearchParams(location.search).get("lang") === "en" ? "en" : "pt"), // ?lang=en
    get_history: () =>
      [["Tarde em Itapoã", "Vinicius de Moraes", 12], ["Águas de Março", "Tom Jobim & Elis Regina", 9], ["Garota de Ipanema", "Stan Getz", 7], ["Construção", "Chico Buarque", 4], ["Ela é Carioca", "Tom Jobim", 2], ["Chega de Saudade", "João Gilberto", 1]].map(
        ([title, artist, plays], i) => ({ title, artist, album: "", coverPath: "mock-cover", plays, lastMs: Date.now() - i * 3_600_000 }),
      ),
    read_cover: async () => {
      const c = document.createElement("canvas");
      c.width = c.height = 300;
      const g = c.getContext("2d")!;
      const grad = g.createLinearGradient(0, 0, 300, 300);
      grad.addColorStop(0, "#1b6b6f");
      grad.addColorStop(1, "#e8c36a");
      g.fillStyle = grad;
      g.fillRect(0, 0, 300, 300);
      g.fillStyle = "#0d0a07";
      g.beginPath();
      g.arc(150, 150, 70, 0, 7);
      g.fill();
      return new Promise<ArrayBuffer>((res) => c.toBlob((b) => b!.arrayBuffer().then(res), "image/png"));
    },
    copy_share_image: (a) => console.info("[mock] copy_share_image bytes:", (a as unknown as Uint8Array).byteLength),
    save_share_image: (a) => {
      const bytes = a as unknown as Uint8Array;
      console.info("[mock] save_share_image bytes:", bytes.byteLength);
      (window as unknown as { __lastCard?: string }).__lastCard = URL.createObjectURL(new Blob([bytes as BlobPart], { type: "image/png" }));
      return "C:\Users\voce\Pictures\Vitrolinha\mock.png";
    },
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
    vinyl: (patch: Partial<typeof vinyl>) => {
      vinyl = { ...vinyl, ...patch };
      emit("vinyl", vinyl);
    },
    set: (patch: Partial<NowPlaying>) => {
      now = { ...now, positionMs: livePos(), sampledAtMs: Date.now(), ...patch };
      emit("now-playing", now);
    },
  };
}

if (import.meta.env.DEV) install();
