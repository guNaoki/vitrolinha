# Vitrolinha

**Português** · [English](README.en.md)

A vitrola da sua tela: flyout de música para Windows com cara de toca-discos vintage. Mostra o que está
tocando (qualquer app que apareça nos controles de mídia do Windows) num **popup** ao
trocar de faixa e num **painel** para o segundo monitor.

Tauri 2 (Rust) + TypeScript/CSS puro + Vite.

## Recursos

- Popup ao trocar de faixa, com anterior · tocar/pausar · próxima
- Painel em tela cheia no monitor escolhido ou janela livre (moldura de mini toca-discos)
- **Linha de duração**: acompanha a faixa e, se o player deixa, pula para onde você clicar. Também dá
  para **segurar o braço da agulha e soltar em outro ponto do disco**
- **Cor de destaque**: tirada da capa (padrão), âmbar clássico, sete cores prontas ou qualquer cor numa **roda de cores** (com brilho do fundo e granulado ajustáveis); capa em alta resolução (iTunes Search, opcional)
- Letras sincronizadas (lrclib.net, opcional) com a linha atual acesa
- Efeitos do disco (brilho/poeira que giram, tremor do braço), em 3 níveis; rotação em **16, 33 ou 45 rpm**
- **Som de disco antigo** opcional por cima da música: chiado de superfície, estalinhos, arranhões e o "tec" de uma risca a cada volta do disco, em 3 níveis (gerado na hora, só com o painel aberto)
- Pausado há um tempo, o **disco volta para a capa**
- **Cartão para compartilhar**: uma imagem com a vitrola e a faixa atual (ou os discos mais ouvidos da
  estante), salva em Imagens\Vitrolinha e já copiada para colar no chat
- Estante com os últimos discos que você ouviu (histórico só neste computador)
- **Timer para dormir**: pausa a música depois de 15 a 90 minutos
- Quem está na call de voz do Discord, na tela cheia (opcional, precisa de configuração)
- **"Ouvindo agora" no perfil do Discord** (opcional) e **overlay para OBS** (opcional)
- Volume do sistema (pequeno, na barra de cima), filtro de fonte de mídia (ex.: só YouTube Music), atalhos globais
- Interface em **português e inglês** (segue o idioma do Windows ou escolha no menu)
- Opções organizadas por função, na tela de opções do painel (botão de três linhas) e no menu da bandeja

## Instalar

Baixe o instalador `Vitrolinha_*_x64-setup.exe` na página de
[Releases](https://github.com/guNaoki/vitrolinha/releases) e abra. Instala só para o seu usuário
(não pede administrador), cria o atalho no menu Iniciar e dá para desinstalar pelo Windows.
Também há um `.zip` portátil. Para abrir junto com o Windows, use "Iniciar com o Windows" no
menu da bandeja.

Windows 10 ou 11 de 64 bits. Feito e testado no Windows 11.

> **O Windows avisou "O Windows protegeu o computador"?** O instalador ainda não é assinado
> digitalmente, e o SmartScreen avisa sobre apps novos. Clique em **Mais informações →
> Executar assim mesmo**. Para conferir o arquivo, compare o SHA256 com o de `SHA256SUMS.txt`
> na mesma página; o instalador é compilado pelo GitHub Actions a partir deste código.

## Algo deu errado?

Clique com o botão direito no ícone da bandeja → **Abrir a pasta de logs** e cole o fim do
`vitrolinha.log` numa [issue](https://github.com/guNaoki/vitrolinha/issues/new/choose). O log
traz só erros e estados do app, cada linha com data e hora; não traz o que você ouve. Fica em
`%LOCALAPPDATA%\app.vitrolinha\logs` (até 1 MB, mais o arquivo anterior).

## Compilar do código-fonte

Precisa de Node, Rust e as ferramentas de build do Visual Studio (MSVC).

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1   # instalador em src-tauri\target\release\bundle\nsis
```

Esse script também tira do exe os caminhos da sua máquina (usuário do Windows, pasta do Cargo).
Para uso diário sem o instalador, `npx tauri build --no-bundle` gera só o exe (sempre com
`--no-bundle`: um `cargo build --release` simples gera um exe que aponta para `localhost:1420`) e
`scripts\install.ps1` copia esse exe para `%LOCALAPPDATA%\Programs\Vitrolinha` e cria os atalhos.

## Overlay para OBS

Ligue **Overlay para OBS** no menu da bandeja e, no OBS, adicione uma fonte **Navegador** com o endereço
`http://localhost:7421` (tamanho sugerido 520 × 170; o fundo é transparente). Mostra o disco girando, com
título, artista e progresso, e some sozinho quando nada toca. O servidor atende só neste computador
(127.0.0.1) e só responde a `localhost`.

## "Ouvindo agora" no Discord

Com **Mostrar no Discord o que estou ouvindo** ligado, o seu perfil do Discord mostra o título e o artista
da faixa, com a barra de progresso. Precisa do mesmo Client ID da lista da call (veja abaixo). Vem
desligado, e só funciona com o app do Discord aberto.

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
   e o Secret, grava em `%APPDATA%\app.vitrolinha\discord.json` (arquivo só do Discord, que o
   app não regrava) e liga a opção. Se preferir editar à mão, o arquivo tem as chaves `enabled`,
   `clientId`, `clientSecret` e, se precisar de outro redirect, `redirectUri`.
4. O script já liga **Call do Discord na tela cheia** (dá para desligar no menu da bandeja) e abre
   a Vitrolinha. Com o Discord aberto, ele pede uma autorização: aceite. Vale uma vez; o token é renovado
   sozinho.

**Sem secret (cliente público, PKCE).** Se no app do Discord você ligar **OAuth2 → Public Client**,
o Client Secret não é necessário: deixe-o vazio no script (ou sem a chave `clientSecret` no
`discord.json`) e a Vitrolinha autoriza com PKCE. Como o Client ID não é segredo, isso também permite
embutir um app único no programa (constante `BUILT_IN_DISCORD_CLIENT_ID` em `settings.rs`).

O escopo `rpc` do Discord é restrito, e não está confirmado se um app não aprovado serve contas
que não são a dona dele (ou **App Testers**). Para conferir, monte o pacote com
`scripts\pacote-teste-discord.ps1 -ClientId <id>` e peça a uma conta de fora para testar.

## Desenvolvimento

```powershell
npm install
npm run dev            # só o front-end, no navegador (sem o Rust; os comandos do Tauri são simulados)
cd src-tauri; cargo test
```

Ao mudar as dependências (`Cargo.lock` ou `package-lock.json`), regenere os avisos de licença:
`python -I scripts/third-party.py` (o CI confere se `THIRD-PARTY-LICENSES.txt` está em dia).

## Licença

[MIT](LICENSE). As licenças das dependências e das fontes estão em
[THIRD-PARTY-LICENSES.txt](THIRD-PARTY-LICENSES.txt), que também vai dentro do instalador.
Não é afiliada ao Discord, à Apple (iTunes) nem ao lrclib.

## Privacidade

Nada sai do computador por padrão, exceto, se ligados no menu da bandeja: busca de letras
(artista, título, álbum e duração → lrclib.net) e de capa grande (artista e título →
itunes.apple.com). A estante fica em `%APPDATA%\app.vitrolinha\shelf` e pode ser
desligada e apagada pelo menu. O log de diagnóstico fica só neste computador. Os cartões de compartilhar são feitos e salvos aqui,
e só vão para onde você os colar.

A call do Discord (desligada por padrão) conversa só com o app do Discord no próprio
computador. Na rede: a troca do código de autorização por token (discord.com) e as fotos de
perfil, que o painel carrega do CDN do Discord. O client secret e o token ficam em texto no
`discord.json`, na pasta de dados do app. "Ouvindo agora" (desligado por padrão) envia o título e o artista para o seu próprio app do Discord, que os
mostra no perfil. O overlay para OBS (desligado por padrão) só responde neste computador.
