# ZAGA 0.2.0

Uma nova interface para zagar e um aplicativo que passa a cuidar das próximas atualizações.

## Novidades

* Atualizações automáticas pelo GitHub, com download em segundo plano e verificação SHA256.
* Instalação ao sair do aplicativo, sem interromper a partida. Também é possível atualizar e reiniciar em Geral.
* O atualizador funciona no instalado e no portátil, sem alterar clipes ou configurações.
* Interface renovada, com Inter e Space Grotesk, banner, ícones dos jogos e novos controles na barra superior.
* Início com clipes recentes e filtros que aparecem somente para jogos com clipes salvos.
* Detecção de jogos por executável e janela, incluindo acompanhamento do processo quando perde o foco.
* Integrações experimentais de autoclipagem para CS2, LoL, VALORANT e Fortnite.
* Destaques experimentais para conquistas Steam raras, com filtro de raridade e descarte do histórico antigo.
* Retenção temporária para eventos processados depois da partida, com limites de tempo e espaço.
* Títulos automáticos nos clipes, preservação de renomeações manuais e exportação sem trocar o momento selecionado.

## Downloads

* `ZagaSetup-0.2.0-x64.exe`: instalador para Windows 10 e 11 em 64 bits.
* `Zaga-0.2.0-portatil-x64.zip`: versão portátil, com executável e licenças.
* `ZAGA-0.2.0-x64.exe`: executável utilizado pelo atualizador.
* `SHA256SUMS.txt`: hashes dos três arquivos.

Quem está na versão 0.1.0 precisa baixar e instalar a 0.2.0 uma vez. As próximas versões poderão ser recebidas
pelo atualizador. Fechar a janela só oculta o ZAGA; escolha Sair na bandeja para aplicar uma atualização pronta.

## Validação e limites

95 testes automatizados passaram. As integrações de autoclipagem continuam experimentais e ainda precisam de
validação em partidas reais. VALORANT depende de interfaces privadas da Riot; Fortnite exige replays habilitados.
As mudanças não comprovam compatibilidade com todos os jogos, drivers ou anticheats.

O instalador permanece sem assinatura digital. A captura grava o monitor principal, inclusive ao alternar para
outros aplicativos. Os clipes ficam no computador e não exigem conta.
