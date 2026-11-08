# Como o Medal detecta jogos e cria clipes automáticos

Investigação da instalação local em 6 de novembro de 2026.

## Conclusão

Nos caminhos examinados, a autoclipagem do Medal é baseada em eventos estruturados específicos de cada jogo.
LoL consulta a API local da partida. CS2 recebe Game State Integration. VALORANT combina logs e detalhes
obtidos depois da partida para recortar a gravação preservada.

Não foi encontrado um detector visual universal nesses três caminhos. Isso não significa que o Medal
inteiro dispense reconhecimento visual ou outros modelos em funcionalidades não examinadas.

O desenho tem quatro partes distintas: reconhecer o jogo, escolher sua integração, agrupar eventos e exportar
o trecho correspondente. Essas partes podem ser reimplementadas de forma independente dentro do processo único
do Zaga. Identificar o executável, sozinho, não entrega autoclipagem.

## O que foi analisado

* Aplicativo Electron: versão `2645.618.1`, obtida do `package.json` distribuído no ASAR.
* Gravador: versão de assembly `2645.3375.1.0`, pasta `recorder-2645.3375.1`.
* Catálogo: versão `2.2188`, ambiente `production`, conforme `GameDBVersion.json` distribuído.
* Artefatos principais: `app.asar`, `MedalEncoder.exe`, `Recorder.Shared.dll`, `events.json` e bancos de jogos.

O REA classificou o launcher e analisou os módulos JavaScript selecionados. A análise dos metadados da DLL
principal encontrou o limite de representação de 64 MiB do REA. A investigação foi complementada com
decompilação estática de tipos selecionados usando ILSpy 9.1.0.7988.

As conclusões sobre as integrações vêm dos corpos de métodos recuperados, além dos nomes de arquivos e símbolos.
Não houve instrumentação dos processos, abertura de jogos ou teste de kills em uma partida real.
Não foram lidos arquivos de autenticação, vídeos pessoais ou históricos privados de partidas.
As configurações efetivamente habilitadas na sessão do usuário não foram verificadas.

## 1. Detecção de jogos

O `ActiveGameManager` cria um `GameScanner` e acompanha seus resultados. O `WindowScanner` enumera janelas
válidas, sem se limitar à janela em primeiro plano. Há uma varredura periódica configurada para dez segundos
e uma nova tentativa agendada cem milissegundos depois da criação de um processo.

O `ProcessTracker` tenta acompanhar criação e encerramento por WMI, com `Win32_ProcessStartTrace` e
`Win32_ProcessStopTrace`. Existe uma alternativa por varredura quando essa inicialização falha.

O reconhecimento consulta um catálogo. O método `FindGameInfosByProcessName` relaciona `GameInfo` e `GameFiles`
pelo identificador do jogo e filtra o nome do processo. Há um caminho complementar com Steam AppID.

Os critérios incluem nome do processo, classe da janela, título, correspondências parciais e casos especiais
para UWP, Chrome e janela principal. Existem filtros de processo, título, classe e tamanho para reduzir
falsos positivos de launchers e janelas transitórias.

Depois da identificação, o código mantém alvos associados a PID e janela. Publica eventos distintos de processo
iniciado, janela aberta, foco, janela fechada e processo encerrado. Nem perder o foco significa fechar o jogo,
nem qualquer janela daquele executável é automaticamente aceita como a janela certa.

Os bancos distribuídos são abertos com configuração de cifra. A leitura comum com SQLite retornou
`SQLITE_NOTADB`. Não foi realizada descriptografia ou contagem completa de jogos. A estrutura acima foi
recuperada das consultas e regras presentes no código.

Fontes: `ActiveGameManager`, `WindowScanner`, `GameScanner`, `ProcessTracker`, `GameInfoDBConnection`
e implementações de `LaunchType`, recuperadas de `Recorder.Shared.dll`.

## 2. Como o evento vira um clipe

Cada integração deriva de `BaseGameCustomization`, que distingue os modos `None`, `RealTime` e `PostMatch`.
O `EventManagementSystem` recebe eventos com identificação, instante e metadados. Confere o alvo atualmente
gravado, as preferências do usuário, o evento habilitado e o modo de jogo antes de solicitar o clipe.

No caminho em tempo real, `TriggerLiveClip` abre uma janela temporal e acumula os eventos em uma fila.
Quando chega outro evento, estende a espera. Ao encerrar o agrupamento, calcula início e duração e envia
`EncoderEvent.SaveClip`. Isso permite juntar a sequência em um trecho, em vez de produzir um vídeo separado
para cada eliminação.

Há parâmetros de contexto anterior, janela de agrupamento e duração máxima. Os valores padrão do gerenciador
são quatro segundos de contexto, quatro segundos de janela e dois minutos de duração máxima, mas integrações
podem sobrescrever esses valores. Eles não representam necessariamente as preferências atuais do usuário.

Se estiver gravando a sessão inteira, há um caminho que salva marcadores na sessão. O caminho após a partida
também possui agrupadores por rodada, janela temporal e união de intervalos sobrepostos.

No `RecordingState.SaveClipImpl`, pedidos com início explícito entram na fila de clipes automáticos.
Pedidos sem início explícito seguem para a fila manual. Portanto, a exportação automática tem um intervalo
próprio; não é somente um atalho pressionado pelo aplicativo.

Fontes: `BaseGameCustomization`, `EventManagementSystem.TriggerLiveClip`, `TriggerPostMatchClips`,
`OverlapMergeGrouper` e `RecordingState.SaveClipImpl`.

## 3. League of Legends

O adaptador declara `EventDetectionType.RealTime`. O `LiveClientEventListener` consulta
`https://127.0.0.1:2999/liveclientdata/eventdata?eventID=...` a cada quinhentos milissegundos no laço recuperado.
Mantém o identificador do último evento para solicitar os seguintes.

O handler confere o envolvimento do jogador por campos como autor da kill, vítima, assistentes e autor do ace.
Também trata eventos de multikill e converte os dados para os eventos internos do Medal.
Há código para descartar eventos antigos recebidos no backlog.

Nessa integração, `OnGameStarted` configura 25 segundos de contexto e 35 segundos de janela de eventos.
Os eventos aceitos são encaminhados a `TriggerLiveClip`. O caminho examinado não precisa reconhecer o aviso
de double kill na imagem da tela.

Fontes: `LeagueOfLegends.OnGameStarted`, `LiveClientEventListener.RequestAsync`,
`LiveClientEventHandler.IsPlayerEvent`, `GetEventObjects` e `Handle`.

## 4. Counter Strike 2

O adaptador também declara `RealTime`. O código gera a configuração
`gamestate_integration_medal.cfg` na pasta de configuração do jogo e há um listener HTTP na porta 12761.

O handler recebe os estados, calcula diferenças de contadores de kills e headshots e acompanha mudanças de
SteamID, modo e rodada. No processamento da rodada, promove eventos para `2K`, `3K`, `4K` e `Ace`,
conforme a contagem acumulada e as opções habilitadas.

A classificação observada é baseada na rodada: cinco ou mais kills promovem para ace nesse trecho do código.
Isso não comprova que a regra seja semanticamente ideal para todos os modos. O adaptador também tem
caminhos separados para modos sem rodadas.

Os parâmetros configurados nesse adaptador são cinco segundos de contexto, cinco segundos de janela e
cinco minutos de duração máxima. Há lógica específica para gerar trechos a partir dos eventos da rodada.

Fontes: `CounterStrike2`, `CounterStrike2Listener`, `CounterStrike2Handler.Handle`,
`ProcessCurrentRoundData`, `TriggerRoundBasedClips` e `UpgradeToMultiKill`.

## 5. VALORANT

Esta é a descoberta que altera a proposta anterior: o adaptador examinado declara `PostMatch`.
Ele não depende de reconhecer kills no HUD para esse caminho.

No início, o código tem um leitor do lockfile do cliente e clientes para obter contexto de sessão e partida.
Essas rotinas foram lidas estaticamente; não foram executadas e nenhuma credencial real foi coletada.
O adaptador inicia um listener dos logs do VALORANT.

O `ValorantRoundHandler` usa mensagens de encerramento de rodada para construir referências de tempo.
O `ValorantPostMatchHandler` identifica o fim da partida no log e tenta obter os detalhes da partida,
incluindo sua lista de kills. O cliente remoto usa um endpoint de detalhes sob o domínio `a.pvp.net`.
Esse caminho é uma integração com interfaces privadas do cliente, não uma API pública documentada para
terceiros cuja estabilidade ou autorização eu tenha confirmado.

O processamento ordena as kills, confere autor e vítima contra a identidade do jogador e conta suas
eliminações por rodada. Constrói eventos `TwoK`, `ThreeK`, `FourK` e `Ace`, além de outros tipos.
Depois chama `PostMatchAnalysis`, que agrupa os eventos e solicita os trechos com início e duração explícitos.

Assim, nesse caminho o jogador pode receber clipes automaticamente depois da partida. A descoberta não
comprova que eles apareçam imediatamente depois de cada kill, nem que a integração esteja funcionando na
partida atual de um usuário.

Fontes: `Valorant.OnGameStarted`, `ValorantRoundHandler.Handle`,
`ValorantPostMatchHandler.ProcessMatchEvents`, `ProcessMatchDetailsWithRetryAsync`,
`ValorantEventGrouper` e `ValorantRemoteApiClient.GetMatchDetailsAsync`.

## 6. Por que o buffer do Zaga precisa mudar para esse caminho

`BaseGameCustomization.PostMatchBufferSeconds` tem padrão de 2700 segundos, equivalente a 45 minutos.
Em `TargetWindow.MinClipBufferLength`, o modo `PostMatch` usa esse valor quando há destaques habilitados.
O retorno considera também a duração pedida pelo usuário, selecionando a maior necessidade.
Esse é um requisito calculado no código, não uma medição de quanto vídeo estava guardado na sessão analisada.

O Zaga atual retém dois minutos por padrão. Se os eventos só chegarem no fim de uma partida longa,
o começo da partida já terá sido sobrescrito. Para reproduzir esse desenho, será necessário preservar
gravação por mais tempo em disco, com limite de espaço e limpeza, ou guardar a sessão inteira até concluir
o processamento. Usar histórico após a partida exige esse armazenamento; o buffer curto não basta.

Fontes: `BaseGameCustomization.PostMatchBufferSeconds`, `TargetWindow.MinClipBufferLength`,
`RecordingState` e `ScopePipelineConfig.RecordingBufferOutput`.

## Fortnite: complemento da investigação

O adaptador `MedalEncoder.GameCustomizations.Fortnite.Fortnite` declara `EventDetectionType.PostMatch`.
No início do jogo, configura quinze segundos de contexto, dez segundos de janela de eventos e dois listeners:
um para o log `FortniteGame.log` e outro para os arquivos de replay.

O `FortniteMatchDetectionHandler` procura informações de partida, playlist e ilha no log.
O `FortniteReplayWatcher` monitora `%LOCALAPPDATA%\FortniteGame\Saved\Demos` com filtro `*.replay`.
Ele lê o estado do replay e aguarda sua transição de gravação em andamento para arquivo concluído.
O laço examinado consulta esse estado em intervalos de dez segundos. Replays já concluídos quando
identificados inicialmente são ignorados nesse caminho para evitar processamento de arquivos antigos.

Quando o replay termina, o watcher chama `FortniteReplayHandler`. Esse handler inicia o executável
`Plugins\Fortnite\GameEventParser\GameEventParser.exe`, passando o modo `parser=fortnite-highlights`,
o caminho do replay e um identificador de usuário obtido pelas configurações do Medal.
O processo é iniciado sem janela e possui uma espera máxima de trinta segundos nesse handler.
Não foi executado durante a investigação.

O watcher também verifica `bReplayRecordingEnabled`. Se estiver falso, registra que os replays não serão
salvos e que a autoclipagem não funcionará por esse mecanismo. Isso demonstra uma dependência dos replays
do próprio Fortnite, além do vídeo capturado pelo Medal.

O catálogo distribuído `events.json` lista estes destaques para Fortnite:

* Eliminação de jogador.
* Eliminação sofrida.
* Victory Royale.
* Long Distance Shot.
* Ludicrous Shot.
* Double Kill.
* Multi Kill.

A presença dos eventos no catálogo não comprova que todos estejam ativos ou funcionando na sessão atual.
Não foram recuperados os critérios internos do parser para distância de tiro, intervalo entre eliminações
ou classificação de multikill. Portanto, não são atribuídos valores ou garantias a essas regras.

Há também um caminho separado em `FortniteClipProcessor` que prepara intervalos de clipes existentes e
invoca o mesmo parser para processamento. Esse caminho não deve ser confundido com o handler que recebe
um replay concluído. A ligação interna completa entre o parser nativo, os eventos retornados e a exportação
não foi reconstruída neste complemento.

Para o Zaga, isso aponta uma alternativa baseada em replays, sem exigir reconhecimento visual contínuo.
Como o processamento examinado ocorre após a conclusão do replay, continua necessário preservar o vídeo
por mais tempo. Para manter um único processo, um parser próprio precisaria ser integrado ao aplicativo,
em vez de reproduzir a chamada ao executável auxiliar do Medal.

Evidência REA do parser: `ev_9434c0033c70ff5fb4e926a1ba57953d89f43d6512a9ed92da9132da9982c0e0`.
SHA256: `1b6780514b5217de3c65dee2749669aa4254398d8e74184da74709b8503d5870`.
O REA identificou um PE sem diretório CLI, portanto não havia metadados CIL disponíveis nesse executável.
Não foi concluída sua análise nativa. As conclusões do fluxo vêm dos métodos gerenciados recuperados
estaticamente e do catálogo distribuído, não da execução ou decompilação integral do parser.

## 7. O que eu implementaria no Zaga

1. Transformar a identificação atual em acompanhamento de processos e janelas, com catálogo de regras,
   filtros de launcher e ciclo de vida por PID. O jogo continua identificado quando perde o foco.
2. Criar adaptadores independentes de eventos. Começar com LoL e CS2, cujos caminhos examinados são em tempo real.
3. Criar um coordenador de destaques com identidade do jogador, partida, rodada, deduplicação,
   contexto anterior e posterior e promoção da sequência para o maior destaque.
4. Permitir exportação por intervalo explícito e marcadores, preservando vídeo e áudio do mesmo período.
5. Tratar VALORANT como opção separada após a partida, condicionada à validação técnica da integração
   e ao armazenamento mais longo. Não prometer clipe imediato usando esse caminho.

Não é necessário começar com uma IA analisando continuamente os quadros para reproduzir os três caminhos
documentados aqui. Também não é necessário copiar o código do Medal ou dividir o Zaga em processos distintos.
Uma implementação em Rust pode usar serviços e threads dentro do executável atual.

## Evidências e limites

Hashes SHA256 dos artefatos observados:

* `app.asar`: `2c1832fa80c3349d45ef93be42b855e017b4730fad3f078113f85a174b5d9e95`.
* `Recorder.Shared.dll`: `8262a1c81548ddfe33dc6b257d9b9f9e3288acb3039c969b51a92934dcf6ef79`.
* `MedalEncoder.exe`: `a723512fd452355339ca393ac84722a83bdf1fb79de0311bf8cdc4296fc0a5da`.

Evidências REA:

* `ev_ef9a855885d0e24f07f681aa1df1f27422bf43e1a84a660e33984d577d6f2898`: classificação e metadados do launcher.
* `ev_fece6b5e058c8ca92c3d61533fd264e3ea71e75bda627c3b08d5799cf4300a16`: limite de representação dos membros da DLL.
* `ev_721eb54709dccdd924f5e4839262d0200d396ff90b20e1bab74a971d1c715995`: limite dos metadados da mesma DLL.
* `ev_f5c25a0447d22c261f87bb2cfe553f5dda150d4dcba6b5bf52e3c4ba473ad233`: análise estática de dois módulos
  JavaScript de autoclipagem e configurações, mais o manifesto, extraídos do ASAR.

A leitura inicial do ASAR encontrou divergência entre o hash declarado para um addon SQLite externo e seu
arquivo acompanhante. Isso foi registrado como limitação; não foi tratado como prova de adulteração.
Não se concluiu a análise integral do ASAR. A análise JavaScript focada manteve relações dinâmicas sem resolução,
e um trace literal de configuração não encontrou correspondência. Ele não prova ausência da funcionalidade.

As conclusões dos handlers C# são análise estática auxiliar com ILSpy, não uma decompilação integral bem sucedida
pelo REA nem observação de execução. Os materiais auxiliares ficam em `.tools/medal_rea`, ignorados pelo Git.
O pacote de evidências REA foi exportado para `.tools/medal_rea/evidencias_rea.json`.

Não foram medidos falsos positivos, latência, consumo ou compatibilidade com anticheat. Não foi validado o
resultado audiovisual de um clipe automático real. Esses pontos precisam de testes com partidas autorizadas
antes de anunciar que a implementação do Zaga está pronta.
