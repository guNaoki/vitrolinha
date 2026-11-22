# vinil-flyout

Flyout de música para Windows com cara de toca-discos vintage. Mostra o que está
tocando (qualquer app que apareça nos controles de mídia do Windows) num **popup** ao
trocar de faixa e num **painel** para o segundo monitor.

Tauri 2 (Rust) + TypeScript/CSS puro + Vite.

## Recursos

- Popup ao trocar de faixa, com anterior · tocar/pausar · próxima
- Painel em tela cheia no monitor escolhido ou janela livre (moldura de mini toca-discos)
- Cor de destaque tirada da capa, capa em alta resolução (iTunes Search, opcional)
- Letras sincronizadas (lrclib.net, opcional) com a linha atual acesa
- Efeitos do disco (brilho/poeira que giram, tremor do braço), em 3 níveis
- Estante com os últimos discos que você ouviu (histórico só neste computador)
- Quem está na call de voz do Discord, na tela cheia (opcional, precisa de configuração)
- Volume do sistema, filtro de fonte de mídia (ex.: só YouTube Music), atalhos globais
- Menu na bandeja com todas as configurações

## Usar no dia a dia

```powershell
npx tauri build --no-bundle          # compila (sempre com --no-bundle)
powershell -ExecutionPolicy Bypass -File scripts\install.ps1
```

O script copia o executável para `%LOCALAPPDATA%\Programs\Flyout` e cria atalhos no
menu Iniciar e na Área de Trabalho. Depois é só abrir "Flyout" pelo menu Iniciar. Para
abrir junto com o Windows, use "Iniciar com o Windows" no menu da bandeja.

## Call do Discord na tela cheia

Mostra no canto de cima à esquerda do painel (só no modo "ocupar o monitor inteiro") quem
está no canal de voz em que você está: avatar, nome, brilho em quem está falando e ícone de
mudo/surdo. Só leitura: não controla a call. Não aparece em janela livre.

O Discord só entrega isso a um app autorizado, então é preciso criar um app **seu** (grátis,
uma vez):

1. Em <https://discord.com/developers/applications> → **New Application** (nome livre).
2. **OAuth2** → em **Redirects**, adicione `http://localhost` e salve. Copie o **Client ID** e,
   em **Reset Secret**, o **Client Secret**.
3. Rode `powershell -ExecutionPolicy Bypass -File scripts\discord-setup.ps1`: ele pede o Client ID
   e o Secret, grava em `%APPDATA%\app.flyout.music\discord.json` (arquivo só do Discord, que o
   app não regrava) e liga a opção. Se preferir editar à mão, o arquivo tem as chaves `enabled`,
   `clientId`, `clientSecret` e, se precisar de outro redirect, `redirectUri`.
4. O script já liga **Call do Discord na tela cheia** (dá para desligar no menu da bandeja) e abre
   o Flyout. Com o Discord aberto, ele pede uma autorização: aceite. Vale uma vez; o token é renovado
   sozinho.

O escopo `rpc` do Discord é restrito: o app só funciona para a conta dona dele (ou contas
adicionadas em **App Testers** no portal), o que basta para uso pessoal.

## Desenvolvimento

```powershell
npm install
npm run dev            # só o front-end, no navegador (sem o Rust)
cd src-tauri; cargo test
```

## Privacidade

Nada sai do computador por padrão, exceto, se ligados no menu da bandeja: busca de letras
(artista, título, álbum e duração → lrclib.net) e de capa grande (artista e título →
itunes.apple.com). A estante fica em `%APPDATA%\app.flyout.music\shelf` e pode ser
desligada e apagada pelo menu.

A call do Discord (desligada por padrão) conversa só com o app do Discord no próprio
computador. Na rede: a troca do código de autorização por token (discord.com) e as fotos de
perfil, que o painel carrega do CDN do Discord. O client secret e o token ficam em texto no
`discord.json`, na pasta de dados do app.
