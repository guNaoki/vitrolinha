// Tela de opções do painel: o menu da bandeja desenhado com a cara do app. O Rust manda o menu como
// dados (tray.rs `model`) e cada clique volta pelo mesmo caminho do menu da bandeja; a tela só
// redesenha o estado novo. Nada fica guardado aqui, só quais seções estão abertas.
//
// Organização (princípios de UX para telas de configuração): opções agrupadas por função, em
// seções que abrem e fecham (revelação progressiva: só o grupo em uso fica aberto, os outros
// mostram o valor atual no título); ligar/desligar em interruptores, escolha entre poucas
// opções em botões marcáveis; rótulo curto com a explicação numa segunda linha discreta; o que
// se usa mais fica no topo e o que é raro ou destrutivo (logs, sair) fica no pé.
import { getMenu, menuClick, type MenuEntry } from "./ipc";
import { t } from "./i18n";

type Sub = Extract<MenuEntry, { kind: "sub" }>;
type Check = Extract<MenuEntry, { kind: "check" }>;
type Item = Extract<MenuEntry, { kind: "item" }>;
type Real = Exclude<MenuEntry, { kind: "separator" }>;

/** Seções por função, na ordem de uso. `ids` são os ids do menu (ver tray.rs). */
const SECTIONS: { id: string; title: string; ids: string[] }[] = [
  { id: "play", title: "Reprodução", ids: ["sources", "sleep"] },
  { id: "disc", title: "Disco e som", ids: ["fx", "vinyl-rpm", "vinyl-crackle"] },
  { id: "look", title: "Visual e conteúdo", ids: ["dynamic-color", "lyrics", "hd-cover"] },
  { id: "windows", title: "Janelas", ids: ["panel-menu", "popup-menu"] },
  { id: "links", title: "Integrações", ids: ["discord", "discord-presence", "overlay", "shortcuts"] },
  { id: "general", title: "Geral", ids: ["autostart", "language", "shelf", "logs", "quit"] },
];

/** "Letras sincronizadas (consulta lrclib.net)" → ["Letras sincronizadas", "consulta lrclib.net"]. */
function splitHint(label: string): [string, string] {
  const m = /^(.*?)\s*\((.*)\)\s*$/.exec(label);
  return m && m[1] ? [m[1], m[2]] : [label, ""];
}

export class OptionsPanel {
  private open = false;
  private openSection = SECTIONS[0].id;

  constructor(
    private readonly root: HTMLElement,
    private readonly body: HTMLElement,
    private readonly button: HTMLElement,
    private readonly onOpen: () => void,
  ) {
    addEventListener("keydown", (e) => {
      if (e.key === "Escape" && this.open) this.close();
    });
    root.querySelector("#options-close")?.addEventListener("click", () => this.close());
    // Clicar fora (no fundo da janela) fecha.
    root.addEventListener("pointerdown", (e) => {
      if (e.target === root) this.close();
    });
  }

  isOpen() {
    return this.open;
  }

  toggle() {
    if (this.open) this.close();
    else void this.show();
  }

  async show() {
    this.onOpen();
    await this.refresh();
    this.root.hidden = false;
    void this.root.offsetWidth; // fixa o estado inicial para a transição rodar
    this.root.classList.add("show");
    this.button.setAttribute("aria-pressed", "true");
    this.open = true;
  }

  close() {
    this.open = false;
    this.root.classList.remove("show");
    this.button.setAttribute("aria-pressed", "false");
    window.setTimeout(() => {
      if (!this.open) {
        this.root.hidden = true;
        this.body.replaceChildren();
      }
    }, 320);
  }

  /** Busca o estado atual e redesenha, mantendo a rolagem onde estava. */
  async refresh() {
    const model = await getMenu().catch(() => [] as MenuEntry[]);
    const top = this.root.scrollTop;
    this.body.replaceChildren(...this.render(model));
    this.root.scrollTop = top;
  }

  private async click(id: string) {
    await menuClick(id).catch((e) => console.warn(e));
    if (this.open) await this.refresh(); // reflete o novo estado nas marcas
  }

  // ---- seções

  private render(model: MenuEntry[]): HTMLElement[] {
    // Grupo que só tem grupos dentro (ex.: Vinil → Rotação, Som) é desfeito: cada um vai para a
    // seção certa.
    const flat = model.flatMap((e) => (e.kind === "sub" && e.children.every((c) => c.kind === "sub") ? e.children : [e]));
    const byId = new Map<string, Real>();
    for (const e of flat) if (e.kind !== "separator") byId.set(e.id, e);

    const used = new Set<string>();
    const out: HTMLElement[] = [];
    for (const s of SECTIONS) {
      const entries = s.ids.map((id) => byId.get(id)).filter((e): e is Real => !!e);
      entries.forEach((e) => used.add(e.id));
      if (entries.length) out.push(this.section(s.id, t(s.title), entries));
    }
    // Opção nova que ainda não tem seção não some: vai para "Outros".
    const rest = [...byId.values()].filter((e) => !used.has(e.id));
    if (rest.length) out.push(this.section("other", t("Outros"), rest));
    return out;
  }

  private section(id: string, title: string, entries: MenuEntry[]): HTMLElement {
    const open = this.openSection === id;
    const box = el("section", "opt-section");
    box.toggleAttribute("data-open", open);

    const head = el("button", "opt-head") as HTMLButtonElement;
    head.type = "button";
    head.setAttribute("aria-expanded", String(open));
    head.append(text("span", "opt-head-title", title), text("span", "opt-head-sum", this.summary(entries)), el("i", "opt-chevron"));
    head.addEventListener("click", () => {
      // Uma seção aberta por vez: menos coisa na tela de cada vez.
      this.openSection = this.openSection === id ? "" : id;
      this.body.querySelectorAll<HTMLElement>(".opt-section").forEach((s) => {
        const on = s === box && this.openSection === id;
        s.toggleAttribute("data-open", on);
        s.querySelector(".opt-head")?.setAttribute("aria-expanded", String(on));
      });
    });

    const inner = el("div", "opt-body-inner");
    inner.append(...this.entries(entries));
    const wrap = el("div", "opt-body");
    wrap.append(inner);
    box.append(head, wrap);
    return box;
  }

  /** O que aparece no título da seção fechada: os valores escolhidos, ou quantas opções estão ligadas. */
  private summary(entries: MenuEntry[]): string {
    const values: string[] = [];
    let on = 0;
    let toggles = 0;
    const walk = (list: MenuEntry[], top = false) => {
      // Marcas soltas no topo da seção são interruptores; dentro de um grupo só de marcas, é uma escolha.
      const radio = !top && list.length > 0 && list.every((c) => c.kind === "check");
      for (const e of list) {
        if (e.kind === "check" && !radio) {
          toggles++;
          if (e.checked) on++;
        } else if (e.kind === "check" && e.checked) {
          values.push(splitHint(e.label)[0]);
        } else if (e.kind === "sub") walk(e.children);
      }
    };
    walk(entries, true);
    if (toggles === 1) values.push(t(on ? "ligado" : "desligado"));
    else if (toggles > 1) values.push(t("{on} de {n} ligados", { on, n: toggles }));
    return values.slice(0, 3).join(" · ");
  }

  // ---- conteúdo

  private entries(list: MenuEntry[]): HTMLElement[] {
    const out: HTMLElement[] = [];
    let toggles: HTMLElement | null = null; // interruptores seguidos formam um bloco só
    let actions: HTMLElement | null = null;
    for (const e of list) {
      if (e.kind === "check") {
        toggles ??= out[out.push(el("div", "opt-toggles")) - 1];
        toggles.append(this.switch(e));
      } else if (e.kind === "sub") {
        out.push(this.group(e));
        toggles = null;
      } else if (e.kind === "item") {
        actions ??= out[out.push(el("div", "opt-actions")) - 1];
        actions.append(this.action(e));
      }
    }
    return out;
  }

  private group(g: Sub): HTMLElement {
    const box = el("div", "opt-group");
    const [title, hint] = splitHint(g.label);
    const head = el("div", "opt-title");
    head.append(text("h3", "", title));
    if (hint) head.append(text("span", "opt-hint", hint));
    box.append(head);

    const kids = g.children;
    if (kids.length && kids.every((c) => c.kind === "check")) {
      // Escolha entre poucas opções: botões marcáveis.
      box.append(this.chips((kids as Check[]).map((c) => this.chip(c.label, c.checked, c.enabled, c.id))));
    } else if (kids.length && kids.every((c) => c.kind === "item")) {
      box.append(this.chips((kids as Item[]).map((c) => this.chip(c.label, false, c.enabled, c.id))));
    } else {
      // Misto: interruptores e ações primeiro, grupos menores depois.
      const order = { check: 0, item: 1, sub: 2, separator: 3 } as const;
      const sorted = [...kids].sort((a, b) => order[a.kind] - order[b.kind]);
      box.append(...this.entries(sorted));
    }
    return box;
  }

  private chips(items: HTMLElement[]): HTMLElement {
    const row = el("div", "opt-chips");
    row.append(...items);
    return row;
  }

  private chip(label: string, on: boolean, enabled: boolean, id: string): HTMLElement {
    const b = text("button", "chip", splitHint(label)[0]) as HTMLButtonElement;
    b.type = "button";
    b.disabled = !enabled;
    b.setAttribute("aria-pressed", String(on));
    b.addEventListener("click", () => void this.click(id));
    return b;
  }

  private switch(c: Check): HTMLElement {
    const [label, hint] = splitHint(c.label);
    const row = el("button", "opt-toggle") as HTMLButtonElement;
    row.type = "button";
    row.disabled = !c.enabled;
    row.setAttribute("role", "switch");
    row.setAttribute("aria-checked", String(c.checked));
    const words = el("span", "opt-words");
    words.append(text("span", "opt-label", label));
    if (hint) words.append(text("span", "opt-hint", hint));
    row.append(words, el("i", "opt-switch"));
    row.addEventListener("click", () => void this.click(c.id));
    return row;
  }

  private action(i: Item): HTMLElement {
    const b = text("button", "opt-action", splitHint(i.label)[0]) as HTMLButtonElement;
    b.type = "button";
    b.disabled = !i.enabled;
    b.addEventListener("click", () => void this.click(i.id));
    return b;
  }
}

function el(tag: string, cls: string): HTMLElement {
  const e = document.createElement(tag);
  e.className = cls;
  return e;
}

function text(tag: string, cls: string, content: string): HTMLElement {
  const e = el(tag, cls);
  e.textContent = content;
  return e;
}
