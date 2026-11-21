# Instala o Flyout para uso diário: copia o executável compilado para uma pasta fixa
# e cria atalhos (menu Iniciar e Área de Trabalho). Depois é só abrir pelo menu
# Iniciar ("Flyout"), sem ir à pasta do projeto, e a pasta de build pode ser apagada.
#
# Uso (na raiz do projeto, depois de `npx tauri build --no-bundle`):
#   powershell -ExecutionPolicy Bypass -File scripts\install.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\install.ps1 -SemAtalhoNaArea   # só menu Iniciar
param([switch]$SemAtalhoNaArea)
$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot -Parent
$origem = Join-Path $root 'src-tauri\target\release\flyout.exe'
if (-not (Test-Path $origem)) { throw "Não achei $origem. Compile antes: npx tauri build --no-bundle" }

$destinoDir = Join-Path $env:LOCALAPPDATA 'Programs\Flyout'
$destino = Join-Path $destinoDir 'flyout.exe'

# Um Flyout aberto trava o arquivo: fecha antes de copiar.
Get-Process flyout -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 1

New-Item -ItemType Directory -Force $destinoDir | Out-Null
Copy-Item $origem $destino -Force

$shell = New-Object -ComObject WScript.Shell
function Novo-Atalho([string]$caminho) {
    $atalho = $shell.CreateShortcut($caminho)
    $atalho.TargetPath = $destino
    $atalho.WorkingDirectory = $destinoDir
    $atalho.Description = 'Flyout de Música'
    $atalho.Save()
    "atalho: $caminho"
}
Novo-Atalho (Join-Path ([Environment]::GetFolderPath('Programs')) 'Flyout.lnk')
if (-not $SemAtalhoNaArea) { Novo-Atalho (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Flyout.lnk') }

Start-Process $destino
"instalado em $destino"
