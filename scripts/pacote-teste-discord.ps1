# Monta um .zip para testar a autorização do Discord com OUTRA conta (a que não é dona
# nem "App Tester" do seu app). Leva o vitrolinha.exe, um discord.json só com o Client ID (sem
# secret, então usa PKCE) e um abrir.ps1 que instala esse arquivo e abre o app.
#
# Antes, no Developer Portal do app: OAuth2 → ligue "Public Client" e confira que
# http://localhost está em Redirects. Não adicione a conta de teste em App Testers.
#
# Uso (depois de `npx tauri build --no-bundle`):
#   powershell -ExecutionPolicy Bypass -File scripts\pacote-teste-discord.ps1 -ClientId 123456789012345678
param([Parameter(Mandatory)][ValidatePattern('^\d{15,25}$')][string]$ClientId)
$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot -Parent
$exe = Join-Path $root 'src-tauri\target\release\vitrolinha.exe'
if (-not (Test-Path $exe)) { throw "Não achei $exe. Compile antes: npx tauri build --no-bundle" }

$pasta = Join-Path $root 'teste-discord'
if (Test-Path $pasta) { Remove-Item $pasta -Recurse -Force }
New-Item -ItemType Directory $pasta | Out-Null
Copy-Item $exe (Join-Path $pasta 'vitrolinha.exe')

$semBom = New-Object Text.UTF8Encoding $false
$json = [ordered]@{ enabled = $true; clientId = $ClientId } | ConvertTo-Json
[IO.File]::WriteAllText((Join-Path $pasta 'discord.json'), $json, $semBom)

$abrir = @'
# Instala o discord.json de teste na pasta de dados da Vitrolinha e abre o app.
# Se a pessoa já usa a Vitrolinha, o discord.json dela é guardado como discord.json.bak.
$ErrorActionPreference = 'Stop'
Get-Process vitrolinha -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 1
$dados = Join-Path $env:APPDATA 'app.vitrolinha'
New-Item -ItemType Directory -Force $dados | Out-Null
$alvo = Join-Path $dados 'discord.json'
if (Test-Path $alvo) { Copy-Item $alvo "$alvo.bak" -Force }
Copy-Item (Join-Path $PSScriptRoot 'discord.json') $alvo -Force
$log = Join-Path $PSScriptRoot 'log.txt'
Start-Process (Join-Path $PSScriptRoot 'vitrolinha.exe') -RedirectStandardError $log
"Vitrolinha aberta. Entre numa call do Discord e aceite o pedido de autorização que o Discord mostrar."
"Se não funcionar, mande o conteúdo de $log."
'@
[IO.File]::WriteAllText((Join-Path $pasta 'abrir.ps1'), $abrir, (New-Object Text.UTF8Encoding $true))

$zip = Join-Path $root 'teste-discord.zip'
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $pasta '*') -DestinationPath $zip
"pacote: $zip"
"A pessoa extrai, abre o PowerShell na pasta e roda: powershell -ExecutionPolicy Bypass -File .\abrir.ps1"
