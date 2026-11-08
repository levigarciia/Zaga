# Atualizações do ZAGA

A partir da versão 0.2.0, o aplicativo consulta a última release estável em
https://api.github.com/repos/levigarciia/Zaga/releases/latest.

## Comportamento

* A verificação automática acontece 20 segundos após abrir e a cada seis horas enquanto o ZAGA estiver aberto.
* O download acontece em segundo plano. Não há reinício durante uma partida.
* Ao escolher Sair na bandeja, a captura encerra e aguarda as exportações antes de fechar o processo.
* O aplicador espera a saída do processo e substitui o executável na mesma pasta, preservando clipes e configuração.
* Fechar a janela apenas oculta o ZAGA. Isso não instala a atualização.
* Configurações, Geral permite desativar a automação, verificar novamente e atualizar e reiniciar.
* O procedimento atende ao instalado e ao portátil, desde que a pasta do executável permita escrita.
* A versão 0.1.0 precisa ser atualizada manualmente uma vez, pois ainda não contém o atualizador.

## Integridade e recuperação

Somente releases estáveis com versão maior são aceitas. O atualizador exige o asset
`ZAGA-<versao>-x64.exe`, no caminho exato da release oficial, e o digest SHA256 fornecido pelo GitHub.
Downloads usam HTTPS, limite de tamanho e tempo máximo. Arquivos incompletos ou divergentes não são executados.
O SHA256 é verificado novamente no aplicador e na cópia colocada ao lado do executável.

A substituição usa ReplaceFileW, com backup temporário na mesma pasta. Uma falha que tenha movido o original
para o backup aciona a restauração. Se a própria restauração falhar, o backup é preservado para recuperação.
Se uma nova instância já tiver sido aberta, a aplicação é adiada. O atualizador não força encerramentos.
Bloqueios transitórios de arquivo, inclusive durante a saída do monitor Steam, têm novas tentativas por até 15 segundos.

Downloads e aplicador ficam em `%LOCALAPPDATA%\Zaga\atualizacoes`. Pastas de sessões anteriores são limpas
na próxima abertura. O diagnóstico fica em `atualizacoes.log`, limitado a aproximadamente 1 MiB.
Builds de desenvolvimento não verificam automaticamente. `ZAGA_DADOS`, `ZAGA_PIPE` e `ZAGA_TESTE_SEM_FOCO`
desabilitam verificações e aplicação automática para não interferir nas validações isoladas.

## Publicação

`scripts/gerar_instalador.ps1` lê a versão de Cargo.toml e recusa versões divergentes.
Publique o instalador, o ZIP portátil, o executável de atualização e SHA256SUMS.txt na release correspondente.
O digest do asset é produzido pelo GitHub após o upload.

## Validação

```powershell
cargo test --workspace --locked
.\scripts\verificar_atualizador.ps1
cargo test -p zaga_interface consulta_e_valida_release_publicada -- --ignored
```

O script usa dados e processos isolados, sem iniciar captura ou interface. Ele verifica a espera pela saída,
a substituição de um executável real, a integridade final e a recusa de um arquivo adulterado.
O teste ignorado consulta e baixa a release pública e deve ser executado após sua publicação.

Referências: https://docs.github.com/en/rest/releases/assets e
https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-replacefilew.
