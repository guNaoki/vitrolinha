# vinil-flyout

Flyout de música para Windows com cara de toca-discos vintage. Mostra o que está
tocando (qualquer app que apareça nos controles de mídia do Windows) num **popup** ao
trocar de faixa e num **painel** para o segundo monitor.

Tauri 2 (Rust) + TypeScript/CSS puro + Vite.

## Recursos

- Popup ao trocar de faixa, com anterior · tocar/pausar · próxima
- Painel em tela cheia no monitor escolhido ou janela livre (moldura de mini toca-discos)
- Cor de destaque tirada da capa, capa em alta resolução (iTunes Search, opcional)
- Letras sincronizadas (lrclib.net, opcional) com brilho palavra por palavra
- Efeitos do disco (brilho/poeira que giram, tremor do braço), em 3 níveis
- Estante com os últimos discos que você ouviu (histórico só neste computador)
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
