# Instala a Vitrolinha para uso diário: copia o executável compilado para uma pasta fixa
# e cria atalhos (menu Iniciar e Área de Trabalho). Depois é só abrir pelo menu
# Iniciar ("Vitrolinha"), sem ir à pasta do projeto, e a pasta de build pode ser apagada.
#
# Uso (na raiz do projeto, depois de `npx tauri build --no-bundle`):
#   powershell -ExecutionPolicy Bypass -File scripts\install.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\install.ps1 -SemAtalhoNaArea   # só menu Iniciar
param([switch]$SemAtalhoNaArea)
$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot -Parent
$origem = Join-Path $root 'src-tauri\target\release\vitrolinha.exe'
if (-not (Test-Path $origem)) { throw "Não achei $origem. Compile antes: npx tauri build --no-bundle" }

$destinoDir = Join-Path $env:LOCALAPPDATA 'Programs\Vitrolinha'
$destino = Join-Path $destinoDir 'vitrolinha.exe'

# A Vitrolinha aberta trava o arquivo: fecha antes de copiar.
Get-Process vitrolinha -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 1

# Migração do nome antigo (Flyout). Só faz algo se houver uma instalação antiga.
Get-Process flyout -ErrorAction SilentlyContinue | Stop-Process -Force
$dadosAntigos = Join-Path $env:APPDATA 'app.flyout.music'
$dadosNovos = Join-Path $env:APPDATA 'app.vitrolinha'
if ((Test-Path $dadosAntigos) -and -not (Test-Path $dadosNovos)) {
    Copy-Item $dadosAntigos $dadosNovos -Recurse
    "dados copiados de $dadosAntigos (a pasta antiga foi mantida; pode apagar depois)"
}
$appAntigo = Join-Path $env:LOCALAPPDATA 'Programs\Flyout'
if (Test-Path $appAntigo) { Remove-Item $appAntigo -Recurse -Force; "removido $appAntigo" }
foreach ($pasta in 'Programs', 'Desktop') {
    $atalhoAntigo = Join-Path ([Environment]::GetFolderPath($pasta)) 'Flyout.lnk'
    if (Test-Path $atalhoAntigo) { Remove-Item $atalhoAntigo -Force; "removido $atalhoAntigo" }
}
$run = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
if (Get-ItemProperty $run -Name Flyout -ErrorAction SilentlyContinue) {
    Remove-ItemProperty $run -Name Flyout
    'removido "Flyout" de Iniciar com o Windows (ligue de novo no menu da bandeja)'
}

New-Item -ItemType Directory -Force $destinoDir | Out-Null
Copy-Item $origem $destino -Force

$shell = New-Object -ComObject WScript.Shell
function Novo-Atalho([string]$caminho) {
    $atalho = $shell.CreateShortcut($caminho)
    $atalho.TargetPath = $destino
    $atalho.WorkingDirectory = $destinoDir
    $atalho.Description = 'Vitrolinha, flyout de música'
    $atalho.Save()
    "atalho: $caminho"
}
Novo-Atalho (Join-Path ([Environment]::GetFolderPath('Programs')) 'Vitrolinha.lnk')
if (-not $SemAtalhoNaArea) { Novo-Atalho (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Vitrolinha.lnk') }

Start-Process $destino
"instalado em $destino"
