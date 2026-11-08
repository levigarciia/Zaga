# Autoclipagem inteligente no Zaga

Análise em 6 de novembro de 2026.

Este documento preserva a pesquisa inicial. A implementação experimental da versão 0.2.0 e seus limites
estão descritos em [estado da implementação](IMPLEMENTACAO_AUTOCLIPAGEM.md).

Atualização: a investigação dos artefatos locais do Medal confirmou caminhos específicos para LoL, CS2
e VALORANT. O [relatório do Medal](MEDAL.md) detalha as evidências e corrige a proposta inicial de VALORANT:
há um caminho após a partida baseado em logs e detalhes do jogo, com retenção de vídeo mais longa.

## Conclusão

É viável salvar automaticamente double kills, triple kills e aces usando o buffer que o Zaga já mantém.
O desafio principal é reconhecer o evento e confirmar que ele pertence ao jogador local.
Não há um detector universal implementado nesta versão.

A recomendação é começar com integrações por jogo, aproveitando eventos documentados quando disponíveis.
Reconhecimento visual pode ampliar o suporte depois, com avaliação própria para cada interface e idioma.
Detectar o executável somente identifica o jogo. Isso não informa quantas eliminações ocorreram.

## O que já existe

O motor grava vídeo e áudio em um buffer circular e remonta os pacotes em MP4 sem recodificar.
Os comandos de salvamento já são compartilhados pelos atalhos, bandeja e interface.
A identificação pelo executável agora funciona dentro do mesmo processo da gravação.

Ainda faltam fontes de eventos, regras de sequência, agendamento do trecho posterior à jogada,
deduplicação, preferências de autoclipagem e validação em partidas reais.

## League of Legends

A Riot documenta a Live Client Data API local, com dados da partida em andamento e o endpoint
`https://127.0.0.1:2999/liveclientdata/eventdata`. A documentação também oferece exemplos de eventos.
É uma base concreta para um primeiro protótipo, desde que os eventos e a identidade do jogador sejam
validados na versão atual do cliente.

Proposta: consultar eventos em intervalos curtos, consumir cada identificador apenas uma vez e aceitar
somente eliminações atribuídas ao jogador local. Usar o evento de sequência oficial quando disponível,
em vez de inventar um intervalo para double kill. Reconexão e reinício da partida exigem nova sessão.

Um ace pode ser do time, sem que o jogador local tenha eliminado todos os adversários.
A preferência deve distinguir ace do time de uma sequência individual.

Fonte: [Game Client API e Live Client Data API da Riot](https://developer.riotgames.com/docs/lol#game-client-api).
O produto também precisa observar os requisitos de registro indicados nessa documentação.

## Counter Strike 2

Game State Integration é o candidato a investigar para receber o estado local sem acessar memória do jogo.
A integração depende de configuração no diretório do jogo e de um receptor HTTP local.
A página oficial da Valve respondeu com erro 403 durante esta pesquisa. Portanto, o contrato e a disponibilidade
dos campos no CS2 atual ainda precisam de confirmação antes de implementar ou anunciar suporte.

Hipótese de implementação: acompanhar eliminações do jogador na rodada, resetar na troca de rodada e
agrupar a sequência até seu encerramento. Duas eliminações na mesma rodada não significam necessariamente
uma double kill com intervalo curto. Um ace exige confirmar modo, equipe e total de adversários da rodada.
Também é preciso diferenciar jogar de assistir outra pessoa.

Referência para validação: [Game State Integration da Valve](https://developer.valvesoftware.com/wiki/Counter-Strike:_Global_Offensive_Game_State_Integration).

## VALORANT

As APIs públicas listadas pela Riot oferecem partidas, histórico, conteúdo e classificação.
A documentação consultada não apresenta um fluxo local público de eliminações em tempo real para esse uso.
Histórico de partidas não resolve a captura imediata usando um buffer de dois minutos.

Reconhecimento visual do HUD é uma alternativa de pesquisa. Deve confirmar as eliminações do jogador,
o contexto da rodada e o encerramento da sequência. Mudanças de HUD, resolução, idioma, efeitos visuais
e modo espectador podem produzir falsos positivos ou perder eventos.

A investigação posterior do Medal encontrou outra alternativa: acompanhar os logs para estabelecer
o contexto temporal, obter os detalhes de kills depois da partida e recortar uma gravação mais longa.
O código examinado usa interfaces privadas do cliente. Isso não comprova estabilidade, autorização oficial
ou funcionamento atual, mas demonstra um desenho que não depende de reconhecimento visual.
Esse caminho requer preservar mais que os dois minutos atuais e produz os trechos após a partida.

Não há comprovação de precisão ou custo de execução no Zaga para essa abordagem.
Ela não deve ser anunciada como suporte pronto. A política exige registro de produtos voltados a jogadores,
inclusive quando não usam as APIs oficiais.

Fonte: [documentação e políticas de VALORANT](https://developer.riotgames.com/docs/valorant).

## Outros jogos

Cada jogo precisa de uma avaliação própria: eventos oficiais, logs documentados ou sinais visuais.
Áudio pode ajudar a confirmar um evento, mas uma fala de narrador ou voz no chat não comprova uma kill do jogador.
Um modelo de linguagem analisando vídeo continuamente não é necessário para o primeiro protótipo e seu consumo
de GPU pode competir com a partida e a codificação.

## Como encaixar no processo único

O detector do jogo seleciona o adaptador adequado. O adaptador produz um evento normalizado contendo jogo,
partida, rodada, identificador, jogador, tipo e instante monotônico. Uma fila limitada entrega esses eventos
a um coordenador dentro do próprio processo, sem bloquear captura, atalhos ou desenho da interface.

O coordenador verifica identidade, sessão, duplicatas e preferências. Uma sequência crescente atualiza um único
candidato: double kill pode virar triple kill e depois ace, sem gerar três arquivos sobrepostos.
O salvamento acontece após alguns segundos sem progressão ou no encerramento da rodada.

Proposta inicial: guardar 20 segundos anteriores ao primeiro evento e 5 segundos posteriores ao último.
Esses valores são parâmetros de projeto, ainda não medidos. Uma sequência longa precisa respeitar a retenção
disponível ou começar uma exportação antes que os primeiros pacotes sejam sobrescritos.

O salvamento atual seleciona os últimos segundos. Para esse fluxo, o anel precisa aceitar um intervalo explícito
de início e fim, alinhado a um quadro chave, com áudio do mesmo período. Apenas esperar cinco segundos e salvar
uma duração fixa pode perder o começo da jogada. Clipe automático deve ser silencioso e não roubar foco.

Falhas do adaptador devem manter o atalho manual funcionando. Ao desativar o recurso, trocar de partida
ou parar a captura, candidatos pendentes devem ser cancelados ou encerrados de forma explícita.
Não se deve salvar eventos antigos recebidos após reconexão.

## Critérios para dizer que funciona

1. Testar eventos repetidos, fora de ordem, reconexão, troca de rodada e mudança de jogador observado.
2. Validar que double, triple e ace geram um único clipe com a classificação final correta.
3. Decodificar o MP4 em ferramenta independente e conferir início, fim e sincronização de áudio.
4. Medir consumo adicional de CPU e GPU, latência e impacto no tempo de quadro com o jogo aberto.
5. Medir precisão e cobertura em partidas reais, incluindo sessões sem destaques e variações de HUD.

Nenhuma dessas métricas de autoclipagem foi medida nesta entrega. Os testes do catálogo comprovam o
reconhecimento de nomes de executáveis, sem comprovar detecção de kills ou compatibilidade com anticheat.

## Ordem recomendada

Primeiro: validar um protótipo com a API local de League of Legends e eventos reais de uma partida.
Depois: confirmar o contrato atual do CS2 e adicionar o adaptador com testes de rodada e espectador.
Por último: avaliar reconhecimento visual para VALORANT e outros jogos com um conjunto de gravações autorizado.

O usuário pode ter que habilitar a integração e configurar preferências uma vez. Após essa preparação,
o objetivo é salvar os destaques sem pressionar atalhos durante a partida.
