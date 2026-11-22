// Lista da call de voz do Discord (só na tela cheia). Os dados vêm do Rust (discord.rs);
// aqui só desenhamos. Falar/mudo mudam só classes CSS (sem refazer o DOM) para não piscar.
import type { Voice, VoiceMember } from "./ipc";

/** Mais que isso vira "+N". */
const MAX_SHOWN = 8;

const HINTS: Partial<Record<Voice["status"], string>> = {
  setup: "Discord: falta o client id no discord.json (veja o README)",
  authorizing: "Discord: autorize o Flyout na janela do Discord",
  refused: "Discord: não autorizado. Desligue e ligue a opção no menu da bandeja para tentar de novo",
};

// Ícones: microfone cortado (mudo) e fone cortado (surdo).
const ICON_MUTE =
  '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 5a3 3 0 0 1 6 0v6a3 3 0 0 1-6 0zM6 11a6 6 0 0 0 12 0M12 17v3M4 4l16 16"/></svg>';
const ICON_DEAF =
  '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 14v-2a8 8 0 0 1 16 0v2M4 14h3v5H4zM17 14h3v5h-3zM4 4l16 16"/></svg>';

type Row = { li: HTMLLIElement; avatar: HTMLElement; name: HTMLElement; flags: HTMLElement; shownAvatar: string | null };

export class VoiceList {
  private readonly rows = new Map<string, Row>();
  private readonly head: HTMLElement;
  private readonly list: HTMLElement;
  private readonly hint: HTMLElement;
  private readonly more: HTMLElement;

  constructor(private readonly root: HTMLElement) {
    this.head = root.querySelector(".voice-head")!;
    this.list = root.querySelector(".voice-list")!;
    this.hint = root.querySelector(".voice-hint")!;
    this.more = root.querySelector(".voice-more")!;
  }

  render(v: Voice) {
    const hint = HINTS[v.status] ?? "";
    const inCall = v.status === "connected" && v.channel !== null;
    this.root.hidden = !inCall && hint === "";
    this.hint.textContent = hint;
    this.hint.hidden = hint === "";
    document.body.toggleAttribute("data-voice", inCall); // o CSS abre espaço (ex.: estreita a letra)
    this.head.hidden = !inCall;
    this.list.hidden = !inCall;
    this.more.hidden = true;
    if (!inCall) {
      this.rows.clear();
      this.list.replaceChildren();
      return;
    }
    this.head.textContent = v.channel;

    // Quem fala primeiro dentre os que ficam fora do corte (a ordem do Discord se mantém).
    const shown = v.members.slice(0, MAX_SHOWN);
    for (const m of v.members.slice(MAX_SHOWN)) {
      if (m.speaking) shown[shown.length - 1] = m; // não esconde quem está falando
    }
    const keep = new Set(shown.map((m) => m.id));
    for (const [id, row] of this.rows) {
      if (!keep.has(id)) {
        row.li.remove();
        this.rows.delete(id);
      }
    }
    shown.forEach((m, i) => {
      const row = this.rows.get(m.id) ?? this.create(m);
      this.update(row, m);
      if (this.list.children[i] !== row.li) this.list.insertBefore(row.li, this.list.children[i] ?? null);
    });
    const hidden = v.members.length - shown.length;
    this.more.hidden = hidden <= 0;
    this.more.textContent = `+${hidden}`;
  }

  private create(m: VoiceMember): Row {
    const li = document.createElement("li");
    const avatar = document.createElement("span");
    avatar.className = "voice-avatar";
    const name = document.createElement("span");
    name.className = "voice-name";
    const flags = document.createElement("span");
    flags.className = "voice-flags";
    li.append(avatar, name, flags);
    const row: Row = { li, avatar, name, flags, shownAvatar: null };
    this.rows.set(m.id, row);
    return row;
  }

  private update(row: Row, m: VoiceMember) {
    row.li.classList.toggle("speaking", m.speaking);
    row.li.classList.toggle("quiet", m.muted || m.deafened);
    if (row.name.textContent !== m.name) row.name.textContent = m.name; // sempre texto, nunca HTML
    const flag = m.deafened ? ICON_DEAF : m.muted ? ICON_MUTE : "";
    if (row.flags.innerHTML !== flag) row.flags.innerHTML = flag;
    if (row.shownAvatar !== (m.avatarUrl ?? m.name)) {
      row.shownAvatar = m.avatarUrl ?? m.name;
      // Inicial por baixo; a foto (do CDN do Discord) cobre quando carrega.
      row.avatar.replaceChildren(document.createTextNode([...m.name][0]?.toUpperCase() ?? "?"));
      if (m.avatarUrl) {
        const img = new Image();
        img.alt = "";
        img.referrerPolicy = "no-referrer";
        img.onerror = () => img.remove(); // sem rede: fica a inicial
        img.src = m.avatarUrl;
        row.avatar.append(img);
      }
    }
  }
}
