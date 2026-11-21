// Cor de destaque dinâmica: sobrescreve --amber (a transição vem do @property em tokens.css).

export function applyAccent(color: string | null | undefined) {
  const root = document.documentElement.style;
  if (color && /^#[0-9a-f]{6}$/i.test(color)) {
    root.setProperty("--amber", color);
  } else {
    root.removeProperty("--amber"); // volta ao âmbar padrão
  }
}
