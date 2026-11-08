# Autoclipagem na versão 0.2.0

## Implementado

Captura, interface e integrações de partidas trabalham em threads do processo principal.
Conquistas Steam usam uma instância auxiliar oculta do mesmo executável para isolar o SDK por AppID.
Consulte [STEAM.md](STEAM.md) para configuração, investigação do Medal e limites de validação.
O catálogo JSON versionado identifica executáveis e filtra títulos, classes e dimensões de janelas transitórias.
O acompanhamento de PID preserva o jogo detectado ao minimizar ou alternar de janela.

A captura somente com jogo aberto é opcional. Na configuração padrão, o buffer manual continua disponível
fora dos jogos. A fonte de imagem ainda é o monitor principal, e não exclusivamente a janela do jogo.
Uma integração é selecionada para o jogo reconhecido. Execução simultânea de vários jogos não foi validada.

O coordenador recebe eventos com jogador local já filtrado, partida, rodada e instante UTC.
Ele ignora reenvios, limita suas coleções e promove cada sequência para a maior contagem de eliminações.
O primeiro e o último evento delimitam o trecho com o contexto anterior e posterior escolhido pelo usuário.

O título usa o evento e o mapa disponível, como `2K: Inferno`, `4K: Ancient` ou `Ace: Split`.
Sem mapa, usa o nome do jogo. O MP4 recebe um nome válido para Windows e um JSON guarda o título completo,
partida, rodada, jogo e intervalo. Uma renomeação manual posterior do vídeo é respeitada na biblioteca.
Os nomes não usam emojis.

## Integrações

### Counter Strike 2

O aplicativo configura somente seu arquivo `gamestate_integration_zaga.cfg`, preservando configurações
de outras ferramentas. Recebe POSTs na interface local, porta 12762, com token persistido na pasta de dados.
O token não é enviado a serviços externos nem aparece nos logs. Reiniciar somente o Zaga não troca esse token.
Na primeira configuração, pode ser necessário reiniciar o CS2 para que ele carregue o arquivo.

Os estados verificam a identidade do jogador, acompanham contadores e mudanças de rodada e geram os destaques
da rodada encerrada. Os nomes promovem 2K, 3K, 4K e Ace. Modos sem rodadas e eventos especiais não possuem
o mesmo conjunto de regras do Medal nesta implementação.

### League of Legends

Consulta a API local de eventos e dados da partida. Mantém cursor de eventos, descarta backlog antigo e
filtra pelo jogador ativo. A sequência aguarda uma janela de agrupamento de 35 segundos para permitir
que um destaque cresça antes da exportação. O contexto posterior do vídeo é configurado separadamente.

Os tempos são ancorados no relógio da partida. A implementação usa os campos de identidade fornecidos
pelo cliente e precisa de validação nas versões atuais e nos diferentes modos.

### VALORANT

Usa o lockfile do cliente Riot para consultar exclusivamente sua interface local.
As credenciais permanecem em memória durante a consulta. Não são gravadas nos metadados dos clipes ou nos logs.
A verificação de certificado desativada é limitada ao cliente usado nos endereços locais fixos.
Redirecionamentos HTTP são desativados e as consultas remotas mantêm validação TLS.

Acompanha a partida atual e observa o log para detectar seu encerramento. Depois consulta os detalhes,
filtra kills do jogador e agrupa por rodada. Usa o horário de início fornecido nos dados da partida para
localizar as eliminações no vídeo. A disponibilidade e a precisão dessa referência precisam de teste real.
Erros transitórios de rede não são tratados como encerramento da partida.

Depende de interfaces privadas que podem mudar. Ainda não existe comprovação de funcionamento em uma
partida real com esta implementação, nem garantia de compatibilidade com as políticas e proteções atuais.

### Fortnite

Monitora replays e reconhece arquivos em andamento e concluídos. Replays anteriores ao início do monitoramento
são ignorados. O arquivo pode reutilizar um nome: tamanho e data de modificação também fazem parte da identidade.
O horário local do cabeçalho é convertido para UTC usando o fuso do Windows antes de localizar os eventos no
buffer. Isso evita descartar partidas recentes como antigas em máquinas com fuso diferente de UTC.
A conta Epic é procurada em campos locais específicos do log. Se ela não estiver disponível, o usuário pode
informar seu ID em Configurações, Destaques. Sem identidade confirmada não são atribuídas kills ao jogador.

O parser interno lê cabeçalhos, eventos de eliminação e estatística de colocação. Suporta versões de arquivo
3 a 7 e versões de eventos de eliminação 3 a 6 e 10. Eventos cifrados usam a chave contida no próprio replay
concluído. Os tamanhos de textos, chaves, cabeçalhos e eventos são limitados antes de alocar memória.
Não é necessário executar o parser do Medal ou instalar outro programa.

Derrubadas não contam como eliminações completas. Eliminações próximas formam grupos e a colocação de vitória
gera um destaque de vitória. As regras de tiro distante do Medal não foram implementadas, pois não foram
recuperadas nem validadas. Novas versões ou tipos desconhecidos geram erro explícito, sem tentar adivinhar dados.

O recorte de vitória usa a última eliminação completa registrada no replay, independentemente do autor,
para incluir o combate final mesmo quando outro integrante do squad elimina o último adversário.
O contexto configurado é aplicado antes e depois dessa eliminação. Sem eliminações disponíveis, usa o instante
da estatística de colocação. A duração total do replay não define o momento da vitória quando esses eventos existem.

As informações de formato foram consultadas no projeto
[FortniteReplayDecompressor](https://github.com/Shiqan/FortniteReplayDecompressor), sob licença MIT.
A licença e a atribuição acompanham os pacotes. A implementação não inclui código extraído do Medal.
Os testes incluem um replay sintético cifrado. Em 6 de novembro de 2026, o leitor também foi executado nos
100 replays locais disponíveis, sem erros, usando a conta Epic configurada: 232 eliminações em 180 destaques.
Essa leitura não comprova a geração automática do MP4 durante uma nova partida.

## Retenção e armazenamento

O vídeo continua comprimido no arquivo circular em disco. Abrir VALORANT ou Fortnite mantém a retenção manual.
A retenção longa começa quando a integração Riot identifica uma partida atual ou quando um replay Fortnite
em gravação recebe atualizações recentes de tamanho ou data. Fortnite exige a gravação de replays habilitada.
Um replay sem atualizações por mais de 120 segundos deixa de indicar uma partida ativa.
A retenção longa usa padrão de 45 minutos, limitada a 16 GiB por padrão.
Esses limites podem ser ajustados na interface. Não há crescimento ilimitado do arquivo temporário.
O limite de bytes prevalece se a qualidade escolhida exigir mais espaço do que o permitido.

As mudanças de retenção migram os pacotes para outro arquivo circular, sem reiniciar os encoders.
A redução aguarda os eventos pendentes e o término das exportações manuais e automáticas.
Durante a migração, os dois arquivos coexistem e a captura aguarda o acesso ao anel.
Falhas na reserva ou na cópia preservam o buffer anterior. CS2 e LoL continuam usando retenção curta.
Os testes cobrem estados e migração com dados sintéticos. A detecção ainda requer validação em partidas reais.

A reserva verifica espaço livre e mantém uma margem de 512 MiB. Se não puder criar o buffer longo,
tenta manter a captura manual com o buffer curto e exibe um aviso. Os clipes já salvos nunca são apagados
para abrir espaço. O arquivo circular é removido ao fechar seu último handle, incluindo exportações pendentes.

Depois de o processo do jogo encerrar, há tolerância de quatro minutos para integrações após a partida
e de dez segundos para os demais jogos. Isso permite processar o fim antes de liberar o contexto de captura.
Trocas de configuração que afetam a captura ou a retenção reiniciam o buffer. Mudar somente o mínimo de
eliminações, o contexto do destaque ou a conta Epic não exige reiniciar a captura.

Um evento antigo não vira um clipe dos segundos atuais: se o intervalo completo não estiver mais disponível,
a exportação falha com aviso. Vídeo e áudio são selecionados no mesmo intervalo, começando em um quadro chave.

## Validação

Consulte [VALIDACAO.md](VALIDACAO.md) para comandos e evidências. Os testes controlados validam agrupamento,
nomes, seleção histórica, replays sintéticos e exportação com áudio. Partidas reais, clientes privados,
temporada atual do Fortnite e desempenho prolongado ainda precisam de validação.
