# Validação da versão 0.2.0

## Escopo

Esta entrega reúne interface, gravação, bandeja e identificação de jogos em `zaga.exe`.
A interface chama o motor diretamente. O pipe permanece disponível para diagnóstico.
Fechar oculta a própria janela na bandeja. Reabrir reutiliza essa janela. Sair encerra a captura e o aplicativo.
Não existe uma janela GPUI adicional para manter o processo vivo.

## Verificações

* `cargo test --workspace --locked`: 95 testes passaram e quatro testes opcionais ficaram ignorados.
  O teste nativo de exportação sintética foi executado separadamente e passou.
* `cargo fmt --all --check`: formatação verificada.
* `cargo build --release --locked -p zaga_interface`: executável otimizado gerado.
* `scripts/verificar_atualizador.ps1`: aplicador real validado com processo e arquivos isolados,
  incluindo espera pela saída, substituição e recusa de pacote adulterado.
* O catálogo tem regressões para nomes sem distinção de caixa, variantes e processos desconhecidos.
* `scripts/verificar_processo_unico.ps1`: verifica o ciclo da bandeja com captura desativada e dados isolados.
  O parâmetro `IniciarNaBandeja` também verifica a abertura da interface sob demanda.

O teste de processo não rouba foco e não captura a tela ou o microfone.
O instalador e o ZIP portátil incluem o executável e a licença de referência do parser de replay.
Os hashes SHA256 acompanham os downloads.
O conteúdo do instalador foi extraído e seu executável tem o mesmo SHA256 da compilação validada.
O executável do ZIP portátil também foi conferido pelo SHA256.
A instalação e a desinstalação completas em uma máquina limpa não foram exercitadas.

## Limites

Não foram executadas partidas reais para verificar todos os jogos do catálogo.
O reconhecimento pelo executável não comprova compatibilidade com todos os modos, versões ou anticheats.
A alteração da arquitetura não constitui uma nova validação da qualidade audiovisual em partidas reais.
Os testes nativos de microfone e atalho permanecem opcionais.

A autoclipagem foi implementada como integração experimental e está descrita em `IMPLEMENTACAO_AUTOCLIPAGEM.md`.
Ainda não foi validada em partidas reais.
O instalador ainda não tem assinatura digital. A atualização automática e sua validação estão em `ATUALIZACOES.md`.

## Exportação histórica com vídeo sintético

O teste `teste_exportacao::exporta_intervalo_historico_com_audio_e_nome` recebe um MP4 sintético H.264 com
áudio AAC e alimenta o mesmo anel usado pela gravação. Solicita o intervalo de dois a cinco segundos,
apesar de já haver doze segundos de material disponível.

O MP4 resultante foi decodificado com FFmpeg sem erros. Os 91 quadros foram comparados por hash com o
intervalo correspondente da origem e são idênticos. O vídeo resultante tem 3,0333 segundos e o áudio AAC
é estéreo a 48 kHz, com início em 0,005333 segundo e duração de 3,008 segundos.
O JSON preservou o título `4K: Ancient` e o arquivo físico recebeu o nome sanitizado.
O arquivo circular foi removido ao terminar o teste. Nenhuma tela ou microfone foi capturado.

```powershell
$env:ZAGA_VIDEO_SINTETICO = 'C:\caminho\origem.mp4'
cargo test -p zaga_motor teste_exportacao::exporta_intervalo_historico_com_audio_e_nome -- --ignored --nocapture
```

O teste de replay cria um arquivo sintético com eventos cifrados, verifica a identidade do autor,
descarta derrubadas e agrupa duas eliminações do jogador local. O teste da versão 10 verifica jogadores humanos,
derrubadas, indicador inválido, truncamento e recusa de uma versão desconhecida.

Em 6 de novembro de 2026, um diagnóstico temporário executou o mesmo leitor nos 100 replays locais disponíveis.
Todos foram lidos sem erros. Com a conta Epic configurada no Zaga, retornaram 232 eliminações em 180 destaques.
Nenhum replay foi alterado e nenhum MP4 foi exportado nesse diagnóstico. O teste não valida a captura contínua
nem a exportação automática ao terminar uma nova partida.
