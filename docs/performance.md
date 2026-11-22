# Medições de performance

Máquina: Windows 11, 12 núcleos lógicos. Build de **release** gerado com `npx tauri build --no-bundle` (`opt-level = "s"`, LTO, strip, panic abort); exe de 3,8 MB.

> **Atenção ao compilar:** use sempre `npx tauri build --no-bundle`. Um `cargo build --release` direto não embute a interface e o exe abre uma página de erro apontando para `localhost:1420`.

CPU em "% de um núcleo" (100% = um núcleo inteiro): soma do `vitrolinha.exe` + todos os processos do WebView2 (browser, GPU, renderers, utilities). RAM = working set somado.

## Baseline: Tauri "hello world" (mesma máquina, release, renderizando)

| Medida | Valor |
|---|---|
| Processos (app + WebView2) | 8 (7 + o conhost do console do hello) |
| RAM (working set) | 357 MB (≈346 MB sem o conhost) |
| RAM privada | 152 MB |
| CPU parado (30 s) | 0,10% de um núcleo |

É o piso do WebView2: a RAM da Vitrolinha deve ser lida como "piso + extras".

## Vitrolinha (marco 5)

| Cenário | Processos | RAM (WS / privada) | CPU |
|---|---|---|---|
| **Oculto**: painel e popup escondidos, aguardando faixa (30 s) | 8 | 479 MB / 232 MB | **0,68%** de um núcleo (0,06% da máquina) |
| **Painel visível, música pausada** (1080×1920, 2º monitor; 30 s) | 8 | 487 MB / 291 MB | **1,77%** de um núcleo |
| **Popup, animação de entrada** (≈1 s) | 8 | 469 MB / 220 MB | ≈0,15 s de CPU no total: 21% na 1ª meia-segundo, 10% na 2ª, pico de ≈35% em 100 ms |
| Popup visível, parado | — | — | ≈0% |
| Popup, animação de saída | — | — | ≈3% |

Leitura:

- **Meta "~0% de CPU oculto": praticamente atendida** (0,68% de um núcleo; o hello world parado dá 0,10%).
- **RAM oculta**: +122 MB sobre o hello world. A Vitrolinha mantém dois WebViews (painel e popup) carregados e cada um tem seu renderer; é o preço de o popup aparecer sem atraso.
- **Disco girando em release: não medido.** A música estava pausada durante esta rodada e não retomei a reprodução do usuário. Estimativa do teste A/B em build de debug com o servidor de desenvolvimento (mesmos processos do WebView2): 21% a 32% de um núcleo, quase tudo no processo da GPU. Falta repetir em release com a música tocando.

## Método

- App iniciado com um `settings.json` de teste (o do usuário é salvo e restaurado), esperando 12 s para o WebView2 assentar.
- CPU = variação de `TotalProcessorTime` de toda a árvore de processos dividida pelo tempo de relógio.
- Painel: colocado acima das outras janelas sem tirar o foco. Coberto por outra janela o WebView2 para de renderizar (oclusão) e a CPU cai para ≈0,3%, o que não representa o uso real.
- Popup: disparado por uma 2ª execução (`vitrolinha.exe --popup`, tratada pela instância única), amostras a cada ~100 ms.

## Observações

- **Girar o disco a 30 fps não ajudou.** Teste A/B com o painel visível e giro forçado (build de debug), 20 s por rodada, CPU em % de um núcleo:

  | Animação | Rodadas | GPU process |
  |---|---|---|
  | linear (60 fps) | 32, 27, 27 | 17–19% |
  | `steps(54)` (30 fps) | 27, 21 | 17–20% |
  | `steps(18)` (10 fps) | 22, 34 | 16–26% |

  O custo não acompanha a taxa de quadros (nem a 10 fps cai), então o giro ficou linear. O custo parece fixo por quadro apresentado (a janela do painel tem 1080×1920 e é recomposta a cada vsync enquanto há animação), não por posição nova do disco.
- A variação entre rodadas é grande (ver tabela acima), principalmente no processo da GPU. Trate os números do disco como ordem de grandeza.
- Caminhos que ainda poderiam reduzir: painel menor (modo de tamanho fixo em vez de ocupar o monitor vertical), tirar os overlays de grão/vinheta da área animada, ou criar o WebView do popup só na primeira troca de faixa para economizar RAM. Nenhum foi testado.
- Ainda sem medição: monitor com escala de DPI diferente da principal.
