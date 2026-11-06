# Validação da versão 0.1.0

## Escopo

Esta entrega reúne interface, gravação, bandeja e identificação de jogos em `zaga.exe`.
A interface chama o motor diretamente. O pipe permanece disponível para diagnóstico.
Fechar oculta a própria janela na bandeja. Reabrir reutiliza essa janela. Sair encerra a captura e o aplicativo.
Não existe uma janela GPUI adicional para manter o processo vivo.

## Verificações

* `cargo test --workspace --locked`: 55 testes aprovados e 2 testes nativos opcionais ignorados.
* `cargo fmt --all --check`: formatação verificada.
* `cargo build --release --locked -p zaga_interface`: executável otimizado gerado.
* O catálogo tem regressões para nomes sem distinção de caixa, variantes e processos desconhecidos.
* `scripts/verificar_processo_unico.ps1`: verifica o ciclo da bandeja com captura desativada e dados isolados.
  O parâmetro `IniciarNaBandeja` também verifica a abertura da interface sob demanda.

O teste de processo não rouba foco e não captura a tela ou o microfone.
O instalador e o ZIP portátil incluem somente o executável do aplicativo.
Os hashes SHA256 acompanham os downloads.
O conteúdo do instalador foi extraído e seu executável tem o mesmo SHA256 da compilação validada.
O executável do ZIP portátil também foi conferido pelo SHA256.
A instalação e a desinstalação completas em uma máquina limpa não foram exercitadas.

## Limites

Não foram executadas partidas reais para verificar todos os jogos do catálogo.
O reconhecimento pelo executável não comprova compatibilidade com todos os modos, versões ou anticheats.
A alteração da arquitetura não constitui uma nova validação da qualidade audiovisual em partidas reais.
Os testes nativos de microfone e atalho permanecem opcionais.

A autoclipagem de double kill, triple kill e ace está descrita como proposta em `AUTOCLIPAGEM.md`.
Ela não foi implementada nem medida nesta versão.
O instalador ainda não tem assinatura digital. Não há atualização automática nesta versão.
