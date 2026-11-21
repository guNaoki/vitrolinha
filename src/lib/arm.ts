// Ângulo do braço da agulha a partir do progresso da faixa.
//
// O braço é desenhado apontando para +x e rotacionado em torno do pivô:
// α = start + span × p (graus, horário, 0° = eixo +x). É uma aproximação linear
// da trajetória real da ponta entre a borda do disco e o selo.

export type ArmProfile = {
  start: number; // ângulo com p = 0 (início da faixa)
  span: number; // quanto gira até p = 1
  rest: number; // repouso (pausado/parado): levantado e afastado do disco
};

/** Painel: pivô (1130, 90), braço de 640px. */
export const PANEL_ARM: ArmProfile = { start: 113.7, span: 28.9, rest: 100 };

/** Popup: pivô (215, 14), braço de 115px, ponta indo de r≈115 a r≈68 do centro do disco. */
export const POPUP_ARM: ArmProfile = { start: 127.2, span: 31.6, rest: 112 };

export function armAngle(progress: number, profile: ArmProfile): number {
  const p = Math.min(1, Math.max(0, progress));
  return profile.start + profile.span * p;
}

/**
 * Aplica o ângulo. A transição de 1s linear (CSS) faz o movimento contínuo com
 * uma atualização por segundo. `sweep` usa uma varredura mais lenta e suave
 * (troca de faixa, play, pause).
 */
export function setArm(el: HTMLElement, deg: number, lifted: boolean, sweep: boolean) {
  el.classList.toggle("lifted", lifted);
  if (sweep) {
    el.classList.add("sweep");
    window.setTimeout(() => el.classList.remove("sweep"), 950);
  }
  el.style.setProperty("--arm", `${deg.toFixed(2)}deg`);
}
