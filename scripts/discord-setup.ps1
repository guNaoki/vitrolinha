# Grava o Client ID e o Client Secret do seu app do Discord em
# %APPDATA%\app.vitrolinha\discord.json e liga a opção "Call do Discord na tela cheia".
# Fecha a Vitrolinha antes e abre de novo no fim. Não mexe no settings.json.
#
# Uso:
#   powershell -ExecutionPolicy Bypass -File scripts\discord-setup.ps1
$ErrorActionPreference = 'Stop'

$pasta = Join-Path $env:APPDATA 'app.vitrolinha'
$arquivo = Join-Path $pasta 'discord.json'
$exe = Join-Path $env:LOCALAPPDATA 'Programs\Vitrolinha\vitrolinha.exe'

$id = (Read-Host 'Client ID (aba OAuth2 do app no Discord)').Trim()
$segura = Read-Host 'Client Secret (não aparece enquanto você cola; deixe vazio se o app é "Public Client")' -AsSecureString
$secret = [Runtime.InteropServices.Marshal]::PtrToStringBSTR([Runtime.InteropServices.Marshal]::SecureStringToBSTR($segura)).Trim()
if (-not $id) { throw 'O Client ID é obrigatório.' }
if ($id -notmatch '^\d{15,25}$') { throw 'O Client ID tem só números (17 a 20 dígitos). Confira se não copiou outra coisa.' }

Get-Process vitrolinha -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 1

New-Item -ItemType Directory -Force $pasta | Out-Null
# Mantém o que já houver no arquivo, menos o token (de outro app, se houver): vai pedir autorização de novo.
$dados = [ordered]@{}
if (Test-Path $arquivo) {
    $atual = [IO.File]::ReadAllText($arquivo).TrimStart([char]0xFEFF) | ConvertFrom-Json
    foreach ($p in $atual.PSObject.Properties) { if ($p.Name -ne 'token') { $dados[$p.Name] = $p.Value } }
}
$dados['enabled'] = $true
$dados['clientId'] = $id
# Sem secret = cliente público (PKCE): tira um secret antigo do arquivo, senão o app usaria o fluxo clássico.
if ($secret) { $dados['clientSecret'] = $secret } else { $dados.Remove('clientSecret') }
# UTF-8 SEM BOM.
[IO.File]::WriteAllText($arquivo, ($dados | ConvertTo-Json -Depth 5), (New-Object Text.UTF8Encoding $false))

$conferido = [IO.File]::ReadAllText($arquivo) | ConvertFrom-Json
"gravado em $arquivo"
"  clientId     : $($conferido.clientId)"
"  clientSecret : " + $(if ($conferido.clientSecret) { "$($conferido.clientSecret.Length) caracteres (fluxo clássico)" } else { 'nenhum (PKCE, cliente público)' })
"  enabled      : $($conferido.enabled)"

Start-Process $exe
'Vitrolinha aberta. Aceite o pedido de autorização que o Discord vai mostrar.'
