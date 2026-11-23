// Idioma da interface: português (o texto do código/HTML) ou inglês. Mesma ideia do `i18n.rs`:
// o texto original em português é a chave, `t()` devolve o inglês quando o idioma é esse. O que
// não está na tabela fica em português, em vez de quebrar. O idioma é escolhido no menu da
// bandeja ("Automático" segue o do Windows); ao trocar, as janelas recarregam.
import { getLang, onLang } from "./ipc";

let lang: "pt" | "en" = "pt";

const EN: Record<string, string> = {
  // Painel (HTML)
  "AGUARDANDO": "WAITING",
  "Opções": "Options",
  "Opções (as mesmas do ícone da bandeja)": "Options (same as the tray icon)",
  "Fechar": "Close",
  "Reprodução": "Playback",
  "Disco e som": "Record and sound",
  "Visual e conteúdo": "Look and content",
  "Janelas": "Windows",
  "Integrações": "Integrations",
  "Geral": "General",
  "Outros": "Other",
  "{on} de {n} ligados": "{on} of {n} on",
  "ligado": "on",
  "desligado": "off",
  "Minimizar painel": "Minimize panel",
  "Estante de discos": "Record shelf",
  "Ocultar letra": "Hide lyrics",
  "Mostrar letra": "Show lyrics",
  "Diminuir janela": "Shrink window",
  "Ocupar o monitor inteiro": "Fill the whole monitor",
  "Gerar cartão para compartilhar": "Create a card to share",
  "Gerar cartão para compartilhar (salva em Imagens e copia)": "Create a card to share (saves to Pictures and copies)",
  "Silenciar": "Mute",
  "Volume": "Volume",
  "Faixa anterior": "Previous track",
  "Próxima faixa": "Next track",
  "Pausar": "Pause",
  "Tocar": "Play",
  "Posição na música": "Position in the track",
  "Letra": "Lyrics",
  "Call do Discord": "Discord call",
  "Estante": "Shelf",
  "Roda de cores": "Color wheel",
  "Brilho da cor": "Color brightness",
  "Brilho do fundo": "Background glow",
  "Granulado": "Film grain",
  "Gerar cartão": "Create card",
  "Salva e copia uma imagem com os discos mais ouvidos": "Saves and copies an image of your most played records",
  // Estados
  "TOCANDO AGORA": "NOW PLAYING",
  "PAUSADO": "PAUSED",
  "PARADO": "STOPPED",
  // Avisos
  "Cartão copiado e {where}.": "Card copied and {where}.",
  "Cartão {where}.": "Card {where}.",
  "salvo em Imagens\\Vitrolinha": "saved to Pictures\\Vitrolinha",
  "não foi possível salvar o arquivo": "could not save the file",
  "Não foi possível gerar o cartão.": "Could not create the card.",
  // Estante
  "agora há pouco": "just now",
  "há {n} min": "{n} min ago",
  "há {n} h": "{n} h ago",
  "ontem": "yesterday",
  "há {n} dias": "{n} days ago",
  "1 disco": "1 record",
  "{n} discos": "{n} records",
  "Ainda não há discos. Toque uma música e ela aparece aqui.": "No records yet. Play a song and it shows up here.",
  // Call do Discord
  "Discord: falta o client id no discord.json (veja o README)": "Discord: the client id is missing in discord.json (see the README)",
  "Discord: autorize a Vitrolinha na janela do Discord": "Discord: authorize Vitrolinha in the Discord window",
  "Discord: não autorizado. Desligue e ligue a opção no menu da bandeja para tentar de novo":
    "Discord: not authorized. Turn the option off and on in the tray menu to try again",
  // Cartões
  "MINHA ESTANTE": "MY SHELF",
  "Os mais ouvidos": "Most played",
  "NA VITROLA": "ON THE TURNTABLE",
  "Ainda não há discos na estante.": "No records on the shelf yet.",
};

/** Texto na língua da interface; `{nome}` é trocado pelos valores de `vars`. */
export function t(pt: string, vars?: Record<string, string | number>): string {
  let s = lang === "en" ? (EN[pt] ?? pt) : pt;
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
  return s;
}

export const currentLang = () => lang;

/** Traduz o que já está na página (rótulos, dicas e textos fixos do HTML). */
function translateDom() {
  document.querySelectorAll<HTMLElement>("[aria-label],[title]").forEach((el) => {
    for (const attr of ["aria-label", "title"]) {
      const v = el.getAttribute(attr);
      if (v && EN[v]) el.setAttribute(attr, EN[v]);
    }
  });
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    const text = n.nodeValue?.trim();
    if (text && EN[text]) n.nodeValue = n.nodeValue!.replace(text, EN[text]);
  }
}

/** Chamar uma vez, antes do primeiro texto desenhado pelo código: lê o idioma e traduz o HTML. */
export async function initI18n(): Promise<void> {
  lang = await getLang().catch(() => "pt" as const);
  document.documentElement.lang = lang === "en" ? "en" : "pt-BR";
  if (lang === "en") translateDom();
  // Idioma trocado no menu: o mais simples e seguro é recarregar a janela.
  void onLang(() => location.reload());
}
