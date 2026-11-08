# Zaga

Clipador de gameplay para Windows: grava continuamente em um buffer circular e salva os últimos 30, 60 ou 120
segundos em MP4 por um atalho global, sem interromper o jogo.

## Arquitetura

Um único executável, `zaga.exe`, com captura e interface no mesmo processo:

Conquistas Steam usam um processo auxiliar oculto do mesmo executável para isolar o SDK por AppID.
O protocolo e os limites estão em `docs/STEAM.md`.

* `crates/nucleo`: lógica pura e testável (anel de pacotes, seleção de clipes, cadência, relógio de áudio, protocolo).
* `crates/motor`: biblioteca residente (captura, encoders, bandeja, atalhos, identificação de jogos).
* `crates/interface`: executável GPUI com janela aberta sob demanda e chamadas diretas ao motor.

Fechar oculta a própria janela e mantém a captura e a bandeja. Sair encerra o processo após parar a captura.
O pipe `\\.\pipe\zaga_motor` atende ferramentas de diagnóstico. A interface não depende dele.
`ZAGA_PIPE` isola pipe, mutex e classe da bandeja nos testes, sem interferir na instância real.
O catálogo versionado fica em `assets/jogos.json`, com regras em `nucleo/src/jogos.rs`.
O detector enumera janelas e acompanha o PID mesmo minimizado. Capturar somente com jogo aberto é opcional.
As integrações experimentais ficam em `motor/src/autoclipagem`; o coordenador puro fica no núcleo.
O monitor principal continua sendo a fonte do vídeo, inclusive durante mudanças de foco.

Fluxo do vídeo, sem cópias para a CPU até o pacote comprimido:

```
Windows.Graphics.Capture (BGRA) -> D3D11 Video Processor (NV12) -> MFT de hardware -> anel em disco -> MP4
```

O áudio segue por WASAPI loopback, AAC do Windows e o mesmo anel. Vídeo e áudio usam o relógio QPC em unidades de
100 ns. Salvar um clipe apenas remonta os pacotes, sem recodificar.

## Interface

Em Configurações, a aba Captura permite ativar o microfone padrão do Windows. A voz é misturada ao áudio do
jogo em uma única trilha. A aba Atalhos permite clicar em uma combinação e pressionar as novas teclas;
Escape cancela a edição. Combinações indisponíveis no Windows aparecem com aviso.

* `aplicativo.rs`: estado raiz, navegação e sincronização com o motor a cada segundo.
* `views/`: telas Início, Clipes, Configurações, editor e barra lateral.
* `components/`: botões, alternador, seletor, campo de texto e controles de janela.
* `services/`: miniaturas e corte (Media Foundation), player, Lixeira e área de transferência.
* `assets/`: marca e ícones, embutidos no executável. `scripts/gerar_recursos.ps1` os regenera.

## Comandos

```powershell
cargo test -p zaga_nucleo -p zaga_motor -p zaga_interface
cargo build --release --locked -p zaga_interface
.\scripts\gerar_instalador.ps1 -Compilador 'C:\caminho\ISCC.exe'
.\scripts\teste_e2e.ps1 -CapturaSegundos 40 -ClipeSegundos 30
.\scripts\capturar_telas.ps1
.\scripts\verificar_processo_unico.ps1
.\scripts\verificar_atualizador.ps1
```

Os testes nativos opcionais do motor verificam o recebimento de amostras do microfone e o registro de uma
combinação global. Execute explicitamente com `cargo test -p zaga_motor -- --ignored`. Eles não gravam arquivos
nem reproduzem áudio. Esses testes não comprovam a qualidade da voz no MP4 final.

* `ZAGA_DADOS`: redireciona dados e clipes para outra pasta. Use em testes para não tocar nos dados reais.
* `verificar_clipe` (exemplo do motor): decodifica um clipe e imprime métricas, sem exibir o conteúdo.
* `validar_ffmpeg.ps1`: validação independente; requer um FFmpeg portátil em `%TEMP%\zaga_ffmpeg_validador`.
* `ZAGA_TESTE_SEM_FOCO`: abre a interface oculta e sem foco. `capturar_telas.ps1` a usa com a janela fora da tela.
* `zaga.exe --bandeja`: inicia sem janela. A bandeja pode abrir e reabrir a interface no mesmo processo.
* O instalador usa Inno Setup, instala por usuário e preserva clipes e configurações na desinstalação.
* `interface/src/atualizador` verifica releases estáveis e aplica o executável ao sair, depois de aguardar o processo.
  O asset `ZAGA-<versao>-x64.exe` e seu digest SHA256 do GitHub são obrigatórios nas próximas releases.
  `docs/ATUALIZACOES.md` descreve o protocolo, os limites e os testes isolados.
* `gerar_instalador.ps1` usa `target/distribuicao` para não substituir um executável de desenvolvimento aberto.
* `ConfigAutoclipagem` controla os jogos, o contexto, a retenção longa e o limite do arquivo temporário.
  CS2 e LoL recebem eventos durante o jogo. VALORANT e Fortnite fazem processamento após a partida.
* O anel continua limitado por tempo e bytes e é removido ao fechar seu último handle.
  A exportação automática usa um intervalo explícito e falha se ele não estiver retido por completo.
* Cada destaque tem metadados JSON ao lado do MP4. O título da biblioteca pode conter dois pontos;
  o nome físico do arquivo é sanitizado. Renomeações manuais do vídeo prevalecem por usarem outro caminho.
* O parser de replay Fortnite é interno. As informações de formato e os créditos estão em
  `docs/IMPLEMENTACAO_AUTOCLIPAGEM.md` e `docs/licenses/FortniteReplayDecompressor.txt`.

## Decisões que não são óbvias no código

* **Encoder criado pelo CLSID:** o objeto retornado por `MFTEnum2` e `MFTEnumEx` no driver AMD 32.0.32015 aceita
    quadros e nunca produz saída. `CoCreateInstance` com o CLSID do mesmo encoder funciona, e o adaptador é definido
    pelo gerenciador DXGI. Não testado ainda em NVIDIA e Intel.
* **Sem `MF_MPEG4SINK_MOOV_BEFORE_MDAT`:** essa opção corrompeu o `mdat` no sink do Windows. O `moov` fica no fim.
* **Repetição do último quadro:** o WGC só entrega quadros quando a tela muda. Sem repetir a 10 fps, um clipe de tela
    parada ficaria curto e cortaria o áudio.
* **Cadência por grade (`ritmo.rs`):** um intervalo mínimo simples resultaria em 55 fps a 165 Hz e 48 fps a 144 Hz.
* **Áudio por contagem de amostras (`relogio_audio.rs`):** o loopback não entrega pacotes no silêncio e o relógio da
    placa deriva. O carimbo vem da contagem, corrigida contra o QPC.

* **Vídeo em janela popup:** o GPUI desenha com DirectComposition `topmost`, acima de janelas filhas. Por isso o player
    usa uma janela popup de propriedade da principal, sem foco, reposicionada pelo relógio do editor.
* **Corte recusa quadros B:** o sink do Windows descarta quadros com carimbos fora de ordem em passthrough, sem erro.
    Clipes do motor não têm quadros B. Clipes externos que tenham recebem uma mensagem clara.

## Armadilhas

* Nunca rode testes visuais assumindo que a máquina está livre: o usuário pode estar jogando. Não roube foco, não
    toque áudio e não fotografe o monitor principal. `capturar_telas.ps1` já segue essas regras, salvo `-MostrarVideo`,
    que coloca a janela sem foco no monitor secundário por alguns segundos.
* Cliques via `PostMessage` falham se o cursor real estiver sobre a janela: o Windows sintetiza `WM_MOUSEMOVE`.

* Validar clipes sempre decodificando com uma ferramenta independente: metadados corretos não garantem quadros
    legíveis, e foi assim que a corrupção do `mdat` apareceu.
* No PowerShell 5.1, `Get-Content` lê UTF-8 sem BOM como ANSI. Não edite fontes com ele.
* `RegisterHotKey` falha se outro programa já usa a combinação; a falha aparece em `motor.log`.
