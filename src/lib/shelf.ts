// Estante: grade com os últimos discos ouvidos (histórico local, ver history.rs).
// Só desenha quando abre; fechada não existe no DOM (nada decodificado, nada rodando).
import { convertFileSrc } from "@tauri-apps/api/core";
import { getHistory, type ShelfRecord } from "./ipc";
import { t } from "./i18n";

/** "há 5 min", "ontem"... (hora local do usuário). */
export function formatAgo(lastMs: number, nowMs = Date.now()): string {
  const min = Math.max(0, Math.round((nowMs - lastMs) / 60_000));
  if (min < 2) return t("agora há pouco");
  if (min < 60) return t("há {n} min", { n: min });
  const h = Math.round(min / 60);
  if (h < 24) return t("há {n} h", { n: h });
  const d = Math.round(h / 24);
  return d === 1 ? t("ontem") : t("há {n} dias", { n: d });
}

export class Shelf {
  /** Discos mostrados agora (o cartão da estante usa esta lista). */
  records: ShelfRecord[] = [];

  constructor(
    private readonly root: HTMLElement,
    private readonly grid: HTMLElement,
    private readonly count: HTMLElement,
    private readonly button: HTMLElement,
  ) {
    addEventListener("keydown", (e) => {
      if (e.key === "Escape" && this.isOpen()) this.close();
    });
  }

  isOpen(): boolean {
    return this.root.classList.contains("show");
  }

  toggle() {
    if (this.isOpen()) this.close();
    else void this.open();
  }

  async open() {
    const records = await getHistory().catch(() => [] as ShelfRecord[]);
    this.render(records);
    this.root.hidden = false;
    void this.root.offsetWidth; // fixa o estado inicial para a transição rodar
    this.root.classList.add("show");
    this.button.setAttribute("aria-pressed", "true");
    document.body.dataset.shelf = "open";
  }

  close() {
    this.root.classList.remove("show");
    // Depois do esmaecer: esconde e solta as imagens (se não reabriu nesse meio-tempo).
    window.setTimeout(() => {
      if (this.root.classList.contains("show")) return;
      this.root.hidden = true;
      this.grid.replaceChildren();
    }, 320);
    this.button.setAttribute("aria-pressed", "false");
    delete document.body.dataset.shelf;
  }

  private render(records: ShelfRecord[]) {
    this.records = records;
    this.count.textContent =
      records.length === 0 ? "" : records.length === 1 ? t("1 disco") : t("{n} discos", { n: records.length });
    const items = records.map((r) => this.item(r));
    if (items.length === 0) {
      const empty = document.createElement("li");
      empty.className = "shelf-empty";
      empty.textContent = t("Ainda não há discos. Toque uma música e ela aparece aqui.");
      items.push(empty);
    }
    this.grid.replaceChildren(...items);
  }

  private item(r: ShelfRecord): HTMLElement {
    const li = document.createElement("li");
    li.className = "rec";
    li.tabIndex = 0;
    li.title = r.album ? `${r.title} — ${r.artist} (${r.album})` : `${r.title} — ${r.artist}`;

    const vinyl = document.createElement("i");
    vinyl.className = "vinyl";
    const sleeve = document.createElement("div");
    sleeve.className = "sleeve";
    if (r.coverPath) sleeve.style.backgroundImage = `url("${convertFileSrc(r.coverPath)}")`;
    else sleeve.textContent = "♪";

    const meta = document.createElement("div");
    meta.className = "meta";
    const title = document.createElement("p");
    title.className = "rec-title";
    title.textContent = r.title;
    const artist = document.createElement("p");
    artist.className = "rec-artist";
    artist.textContent = r.artist;
    const info = document.createElement("p");
    info.className = "rec-info";
    info.textContent = `${r.plays > 1 ? `${r.plays}× · ` : ""}${formatAgo(r.lastMs)}`;
    meta.append(title, artist, info);

    li.append(vinyl, sleeve, meta);
    return li;
  }
}
