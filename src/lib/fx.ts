// Nível dos efeitos do disco → `body[data-fx]` (o CSS faz o resto).
import { getDiskFx, onDiskFx, type DiskFx } from "./ipc";

const apply = (level: DiskFx) => {
  document.body.dataset.fx = level;
};

/** Lê o nível salvo e acompanha mudanças feitas no menu da bandeja. */
export function initDiskFx() {
  apply("soft"); // padrão até o Rust responder
  void getDiskFx().then(apply).catch(() => {});
  void onDiskFx(apply);
}
