<div align="center">
    <img src="assets/marca/logo.png" alt="Zaga" width="320">
    <p><strong>Sua melhor jogada merece um replay.</strong></p>
    <p>Guarde os últimos segundos da sua gameplay com um atalho, sem interromper a partida.</p>
    <p>
        <a href="https://github.com/levigarciia/Zaga/releases/latest">Baixar para Windows</a>
        · <a href="#como-usar">Como usar</a>
        · <a href="https://github.com/levigarciia/Zaga/issues">Pedir ajuda</a>
    </p>
    <p>Windows 10 e 11 · 64 bits · Vídeos locais em MP4</p>
</div>

## Instale e comece a jogar

1. Abra a [página de downloads](https://github.com/levigarciia/Zaga/releases/latest).
2. Baixe o arquivo `ZagaSetup` com extensão `.exe` e execute o instalador.
3. Abra o Zaga e confirme que o buffer de gravação está ligado.
4. Jogue normalmente. Quando acontecer uma jogada que você quer guardar, pressione o atalho.

Prefere usar sem instalar? Baixe a versão portátil, extraia o ZIP e abra `zaga.exe`.

Esta é uma versão inicial. O instalador ainda não possui assinatura digital e o Windows pode exibir um aviso.

## Como usar

<table>
    <tr><th>Atalho padrão</th><th>O que salva</th></tr>
    <tr><td><kbd>Alt</kbd> + <kbd>F9</kbd></td><td>Últimos 30 segundos</td></tr>
    <tr><td><kbd>Alt</kbd> + <kbd>F10</kbd></td><td>Últimos 60 segundos</td></tr>
    <tr><td><kbd>Alt</kbd> + <kbd>F11</kbd></td><td>Últimos 120 segundos</td></tr>
</table>

Você pode mudar os atalhos nas configurações. Depois de abrir o aplicativo, espere o buffer acumular o tempo
que deseja salvar. Ao pausar a captura ou alterar os parâmetros de gravação, o buffer é reiniciado.

Seus vídeos ficam na pasta **Vídeos\Zaga** do Windows. Na tela **Clipes**, você pode assistir, renomear,
cortar e abrir a pasta dos arquivos.

## Do seu jeito

* Escolha resolução, qualidade, taxa de quadros e codec nas configurações.
* Inclua o áudio do computador e, se quiser, o microfone padrão do Windows.
* Ative a inicialização com o Windows para deixar o Zaga pronto na bandeja.
* Feche a janela e continue jogando. Para encerrar a gravação, escolha **Sair** no ícone da bandeja.

Interface, gravação e identificação de jogos funcionam no mesmo `zaga.exe`.
Os clipes ficam no seu computador. Não é necessário criar conta para gravar.

## Detecção de jogos

O Zaga reconhece executáveis conhecidos em primeiro plano e usa o nome do jogo ao salvar o clipe.
O catálogo inicial inclui CS2, VALORANT, League of Legends, Fortnite, Apex Legends, Rocket League,
Overwatch 2, Rainbow Six Siege, Dota 2, PUBG e GTA V.

Para outros programas, o nome vem do título da janela. A detecção identifica o jogo, mas a gravação atual
continua capturando o monitor principal, inclusive ao alternar para outros aplicativos.
Ela não inicia nem pausa o buffer automaticamente ao entrar ou sair de um jogo.

## E os clipes automáticos de kills?

Esta versão salva por atalho ou pelo botão do aplicativo. Double kill, triple kill e ace ainda não geram
clipes automaticamente. Veja a proposta e as limitações no [relatório de autoclipagem](docs/AUTOCLIPAGEM.md).

## Requisitos e dúvidas

* Windows 10 versão 1903 ou superior, ou Windows 11, em 64 bits.
* GPU e driver com codificação de vídeo por hardware compatível com o codec escolhido.
* Componentes Media Foundation disponíveis. Edições N do Windows podem precisar do Media Feature Pack.
* Espaço em disco para o buffer temporário e os clipes salvos.

**O atalho não funciona:** confira se outro aplicativo usa a mesma combinação e escolha outra nas configurações.

**A captura não inicia:** atualize o driver da GPU, tente H.264 e confira a mensagem exibida no Zaga.
A validação local de hardware foi feita em uma GPU AMD. NVIDIA e Intel ainda precisam de validação real.

**O clipe ficou curto:** o buffer ainda estava acumulando vídeo ou foi reiniciado recentemente.

**Quero desinstalar:** use os Aplicativos Instalados do Windows. Seus clipes e configurações são preservados.

Encontrou um problema? [Abra uma issue](https://github.com/levigarciia/Zaga/issues) com sua versão do Windows,
GPU e mensagem de erro. Evite incluir vídeos ou informações pessoais sem revisar.

<details>
<summary>Para quem desenvolve</summary>

O projeto usa Rust, GPUI e as APIs de mídia do Windows.
Consulte [as instruções técnicas](docs/AGENTS.md) para compilar, testar e gerar o instalador.
O código está disponível para consulta, sem concessão de licença de redistribuição nesta versão.

</details>
