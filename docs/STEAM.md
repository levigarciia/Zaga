# Conquistas da Steam

Em Configurações, Autoclipagem, ative Conquistas raras da Steam. O padrão aceita conquistas com percentual
global abaixo de 10%. Também há opções de 5% e 25%. O contexto anterior e posterior usa os mesmos ajustes
dos demais destaques. O mínimo de eliminações não se aplica às conquistas.

A Steam deve estar aberta com uma conta que tenha acesso ao jogo. A instalação é identificada pelos
arquivos `appmanifest_*.acf` na pasta `steamapps`, incluindo bibliotecas em outros discos. O executável
precisa estar dentro da instalação em `steamapps/common`. Jogos iniciados por executáveis fora dessa
estrutura e instalações sem manifesto ainda não são reconhecidos por esse caminho.

O Zaga inicia uma instância oculta do próprio executável, com `SteamAppId` definido apenas nesse processo.
Ela consulta o SDK a cada três segundos e termina quando o processo principal fecha o canal ou a integração
é desativada. A captura e a interface continuam no processo principal. Não é necessário fornecer senha,
chave Web API nem tornar o perfil público. Não há chamadas de alteração de conquistas.

Cada conquista precisa ser observada bloqueada antes de ser aceita como um novo desbloqueio. Conquistas
antigas, repetidas, com raridade desconhecida ou com desbloqueio recebido mais de quinze segundos depois
do horário informado pela Steam são ignoradas. Se o percentual ainda não chegou, o monitor aguarda dentro
dessa janela. Falhas no monitor recebem nova tentativa após trinta segundos. Reiniciar o monitor cria uma
nova linha de base e pode perder desbloqueios ocorridos durante a recuperação.
O processo principal encerra o monitor se não receber dados durante vinte segundos, inclusive se a
inicialização do SDK ficar bloqueada. Fechar o canal de entrada também encerra o auxiliar.

O título do clipe inclui o nome e o percentual. O JSON do destaque conserva o AppID no campo `partida`,
com prefixo `steam:`, e o identificador da conquista no campo `rodada`. Conquistas simultâneas produzem
clipes separados. A exportação exige que todo o contexto continue disponível no buffer.

## Investigação do Medal

Instalação examinada em 7 de novembro de 2026, gravador `2645.3375.1`.

O REA identificou `Plugins/Steam/MedalSteamUtils.exe`, assembly .NET Framework 4.6.2, MVID
`bfd206b0-0375-4ca7-b140-6db4c00fe639`, SHA256
`ed987508211e83c5bc5052e0f1592c2bdf136a0213bc535808d1051c6bd1d454`.
Evidência: `ev_074730d09b875ff85ecf428b498ba44bd3ac9c336355d1debfeee09276cd843a`.
O inventário de membros também registrou o método assíncrono de acompanhamento, chamadas de enumeração
de conquistas e esperas. Sua cobertura foi parcial. Evidência:
`ev_e2b99f497fc2ddb364c2346dbe028f9d6846bc6adf93faa8fd22c937c270c6d8`.

A leitura de `Recorder.Shared.dll` pelo REA atingiu o limite de representação de 64 MiB.
Evidência: `ev_721eb54709dccdd924f5e4839262d0200d396ff90b20e1bab74a971d1c715995`.
Os corpos dos métodos abaixo foram recuperados estaticamente com o ILSpy 9.1 já disponível no projeto:

* `SteamAchievementListener.StartSteamAchievementWatcher`: inicia `MedalSteamUtils.exe` no modo
  `watch-achievements`, passando o AppID, e recebe JSON pela saída padrão.
* `SteamOperations.WatchAchievements`: inicializa Facepunch Steamworks, aguarda três segundos, guarda
  as conquistas já desbloqueadas e consulta novos desbloqueios a cada três segundos.
* `SteamAchievementConstants.GetEventIdForRarity`: abaixo de 5% é ultrarrara, abaixo de 10% é rara.
* `SteamAchievementHandler.Handle`: filtra a categoria habilitada, agrupa eventos e pede um clipe com
  quinze segundos anteriores, janela posterior de quinze segundos e limite de dois minutos.

Essas são observações de código distribuído. Não houve execução do Medal, desbloqueio de conquista ou
observação de clipe real nessa investigação. O Zaga tem implementação própria e usa os seus ajustes de
contexto, sem reproduzir o agrupamento do Medal.

## Biblioteca distribuída e validação

`assets/steam/steam_api64.dll` foi obtida do pacote `steamworks-sys` 0.13.0, diretório
`lib/steam/redistributable_bin/win64`. A assinatura Authenticode foi verificada como válida, de Valve Corp.
SHA256: `eb17909a76668cf9ae0b92a618a34a50f6c73d3a6787cb4dd8ce36a8b10bfb75`.
A biblioteca é embutida no executável e extraída em uma pasta versionada nos dados do aplicativo.
Os bytes locais são comparados ao recurso embutido antes de carregar a DLL.

Os testes cobrem classificação por manifesto, linha de base, transições, duplicatas, limiar de raridade,
percentual atrasado, eventos antigos, compatibilidade das configurações e exportações da DLL utilizada.
Resultado: 89 testes aprovados e três testes opcionais de hardware ignorados. A compilação de produção
com `cargo build --release --locked -p zaga_interface` concluiu.

O executável compilado foi iniciado no modo de monitor com o AppID 620. O SDK real respondeu com 51
conquistas do Portal 2 e o processo encerrou com código zero quando seu canal de entrada foi fechado.
Outro teste com biblioteca inexistente retornou erro JSON sem abrir a interface.

O primeiro teste do SDK encontrou uma inicialização bloqueada. A implementação recebeu a verificação
prévia do cliente, o limite de vinte segundos sem resposta e o encerramento por fechamento do canal.
A leitura real e os testes não comprovam detecção durante um jogo real. Ainda é necessário desbloquear
uma conquista rara com o Zaga capturando e decodificar o MP4 resultante.

Referência do SDK: [estatísticas e conquistas](https://partner.steamgames.com/doc/api/ISteamUserStats).
