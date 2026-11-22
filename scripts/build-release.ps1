# Compila o release (instalador NSIS e vitrolinha.exe) sem deixar caminhos da sua máquina
# (usuário do Windows, pasta do Cargo/Rustup) dentro do binário: o Rust embute esses caminhos
# nas mensagens de pânico e na informação de depuração. É o que o GitHub Actions roda.
#
# Uso (na raiz do projeto):
#   powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1                 # instalador + exe
#   powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1 -SemInstalador  # só o exe
param([switch]$SemInstalador)
$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot -Parent
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
$rustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { Join-Path $env:USERPROFILE '.rustup' }

# Quando mais de uma regra serve, a última vence: da mais geral para a mais específica.
# CARGO_ENCODED_RUSTFLAGS separa por 0x1F, então caminhos com espaço não quebram.
$regras = @(
    "--remap-path-prefix=$env:USERPROFILE=/user",
    "--remap-path-prefix=$rustupHome=/rustup",
    "--remap-path-prefix=$cargoHome=/cargo",
    "--remap-path-prefix=$root=/src"
)
$env:CARGO_ENCODED_RUSTFLAGS = $regras -join [char]0x1f

Push-Location $root
try {
    npm ci
    if ($LASTEXITCODE -ne 0) { throw 'npm ci falhou' }
    if ($SemInstalador) { npx tauri build --no-bundle } else { npx tauri build }
    if ($LASTEXITCODE -ne 0) { throw 'tauri build falhou' }
} finally {
    Pop-Location
}

$exe = Join-Path $root 'src-tauri\target\release\vitrolinha.exe'
$texto = [Text.Encoding]::GetEncoding(28591).GetString([IO.File]::ReadAllBytes($exe))
if ($texto.IndexOf($env:USERNAME, [StringComparison]::Ordinal) -ge 0) {
    Write-Warning "O nome de usuário '$env:USERNAME' ainda aparece dentro de $exe"
} else {
    'ok: o nome de usuário não aparece no exe'
}
Get-ChildItem (Join-Path $root 'src-tauri\target\release\bundle\nsis') -Filter *.exe -ErrorAction SilentlyContinue | ForEach-Object { "instalador: $($_.FullName)" }
