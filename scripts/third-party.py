"""Gera THIRD-PARTY-LICENSES.txt: o aviso de licença das dependências que vão dentro do app.

Rust: as crates de execução do alvo Windows (sem as de build e de teste), com o texto dos
arquivos de licença que cada uma traz. npm: só o que o front-end embute (@tauri-apps/api).
Fontes: as licenças que já estão em src/assets/fonts. Roda offline (usa o cache do Cargo).

Uso, na raiz do projeto:  python -I scripts/third-party.py
Precisa ter rodado `npm ci` e `cargo fetch` (um build já basta).
"""

import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGET = "x86_64-pc-windows-msvc"
LICENSE_FILE = re.compile(r"^(LICEN[CS]E|COPYING|UNLICENSE|NOTICE)([-_. ].*)?$", re.IGNORECASE)


def run(*cmd, cwd):
    return subprocess.run(cmd, cwd=cwd, check=True, capture_output=True, text=True, encoding="utf-8").stdout


def rust_packages():
    cwd = ROOT / "src-tauri"
    # {p} = "nome versão (origem)"; "--prefix none" lista uma crate por linha; (*) marca repetidas.
    tree = run("cargo", "tree", "-e", "normal", "--target", TARGET, "--prefix", "none", "--format", "{p}", cwd=cwd)
    wanted = set()
    for line in tree.splitlines():
        m = re.match(r"^(\S+) v(\S+)", line.strip())
        if m:
            wanted.add((m.group(1), m.group(2)))
    meta = json.loads(run("cargo", "metadata", "--format-version", "1", "--filter-platform", TARGET, cwd=cwd))
    own = meta["workspace_members"]
    out = []
    for p in meta["packages"]:
        if (p["name"], p["version"]) in wanted and p["id"] not in own:
            out.append(p)
    return sorted(out, key=lambda p: (p["name"].lower(), p["version"]))


def license_texts(directory):
    """(nome do arquivo, texto) dos arquivos de licença na raiz da crate e em licenses/."""
    found = []
    for base in (directory, directory / "licenses", directory / "LICENSES"):
        if not base.is_dir():
            continue
        for f in sorted(base.iterdir()):
            if f.is_file() and LICENSE_FILE.match(f.name):
                found.append((f.name, f.read_text(encoding="utf-8", errors="replace").strip()))
    return found


def main():
    packages = rust_packages()
    by_text = {}  # hash do texto -> {"text", "crates": [rótulos]}
    declared = []
    sem_arquivo = []
    for p in packages:
        label = f'{p["name"]} {p["version"]}'
        lic = p.get("license") or p.get("license_file") or "(não declarada)"
        declared.append((label, lic, p.get("repository") or p.get("homepage") or ""))
        texts = license_texts(Path(p["manifest_path"]).parent)
        if not texts:
            sem_arquivo.append((label, lic))
        for _name, text in texts:
            h = hashlib.sha256(text.encode("utf-8")).hexdigest()
            by_text.setdefault(h, {"text": text, "crates": []})["crates"].append(label)

    out = []
    w = out.append
    w("AVISOS DE LICENÇAS DE TERCEIROS — Vitrolinha")
    w("=" * 60)
    w("")
    w("A Vitrolinha é distribuída sob a licença MIT (arquivo LICENSE). Ela inclui, no")
    w("executável e no instalador, os componentes abaixo, cada um sob a sua própria licença.")
    w("Este arquivo é gerado por scripts/third-party.py.")
    w("")
    w("Crates com MPL-2.0 estão ligadas sem modificação; o código-fonte delas está em")
    w("crates.io e nos repositórios listados na seção 1.")
    w("")
    w("")
    w("1. DEPENDÊNCIAS RUST (%d)" % len(declared))
    w("-" * 60)
    for label, lic, repo in declared:
        w(f"{label} — {lic}" + (f" — {repo}" if repo else ""))
    w("")
    w("")
    w("2. DEPENDÊNCIA DO FRONT-END (npm)")
    w("-" * 60)
    api = ROOT / "node_modules" / "@tauri-apps" / "api"
    pkg = json.loads((api / "package.json").read_text(encoding="utf-8"))
    w(f'@tauri-apps/api {pkg["version"]} — {pkg.get("license", "?")} — https://github.com/tauri-apps/tauri')
    npm_texts = license_texts(api)
    for name, text in npm_texts:
        w("")
        w(f"[{name}]")
        w(text)
    w("")
    w("")
    w("3. FONTES")
    w("-" * 60)
    for f in sorted((ROOT / "src" / "assets" / "fonts").glob("LICENSE-*.txt")):
        w(f"[{f.name}]")
        w(f.read_text(encoding="utf-8", errors="replace").strip())
        w("")
    w("")
    w("4. TEXTOS DAS LICENÇAS DAS CRATES RUST")
    w("-" * 60)
    w("Cada texto aparece uma vez, seguido das crates que o trazem.")
    w("")
    for entry in sorted(by_text.values(), key=lambda e: (-len(e["crates"]), e["crates"][0].lower())):
        w("#" * 60)
        w("Crates: " + ", ".join(entry["crates"]))
        w("#" * 60)
        w(entry["text"])
        w("")
    if sem_arquivo:
        w("")
        w("5. CRATES SEM ARQUIVO DE LICENÇA NO PACOTE")
        w("-" * 60)
        w("Licença declarada no Cargo.toml; o texto-padrão está em https://spdx.org/licenses/.")
        for label, lic in sem_arquivo:
            w(f"{label} — {lic}")

    dest = ROOT / "THIRD-PARTY-LICENSES.txt"
    dest.write_text("\n".join(out) + "\n", encoding="utf-8", newline="\n")
    print(f"{dest.name}: {len(declared)} crates, {len(by_text)} textos distintos, {dest.stat().st_size // 1024} KB")
    if sem_arquivo:
        print(f"{len(sem_arquivo)} crates sem arquivo de licença: " + ", ".join(l for l, _ in sem_arquivo), file=sys.stderr)
    deslicencas = [l for (l, lic, _r) in declared if lic == "(não declarada)"]
    if deslicencas:
        print("SEM LICENÇA DECLARADA: " + ", ".join(deslicencas), file=sys.stderr)


if __name__ == "__main__":
    main()
