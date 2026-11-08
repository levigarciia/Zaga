param(
    [string]$Saida = (Join-Path $env:TEMP 'zaga_telas'),
    [string]$Executavel = 'target\release\ZAGA.exe',
    [switch]$MostrarVideo
)

$raiz = Split-Path -Parent $PSScriptRoot
$dados = Join-Path $env:TEMP 'zaga_teste_telas'
New-Item -ItemType Directory -Force $Saida | Out-Null
Get-ChildItem $Saida -Filter *.png -ErrorAction SilentlyContinue | Remove-Item -Force -Confirm:$false
if (Test-Path $dados) { Remove-Item -Recurse -Force $dados -Confirm:$false }
$clipes = Join-Path $dados 'clipes'
& (Join-Path $raiz 'scripts\preparar_clipes_teste.ps1') -Pasta $clipes
'{"captura_ativa":false}' | Out-File -Encoding ascii (Join-Path $dados 'config.json')
$env:ZAGA_DADOS = $dados
$env:ZAGA_TESTE_SEM_FOCO = '1'

Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class Win {
    public delegate bool Visitante(IntPtr janela, IntPtr dado);
    [DllImport("user32.dll")] public static extern bool EnumWindows(Visitante visitante, IntPtr dado);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr janela, out uint processo);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr janela, StringBuilder texto, int tamanho);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr janela, uint mensagem, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr janela, IntPtr hdc, uint opcoes);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr janela, out RECT area);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr janela, int x, int y, int largura, int altura, bool redesenhar);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr janela, int comando);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr janela);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr janela);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr janela, IntPtr ordem, int x, int y, int largura, int altura, uint opcoes);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr janela, StringBuilder nome, int tamanho);
    public static IntPtr EncontrarPorClasse(uint alvo, string classe) {
        IntPtr achada = IntPtr.Zero;
        EnumWindows((janela, dado) => {
            uint processo;
            GetWindowThreadProcessId(janela, out processo);
            if (processo != alvo) return true;
            var nome = new StringBuilder(64);
            GetClassName(janela, nome, 64);
            if (nome.ToString() != classe) return true;
            achada = janela;
            return false;
        }, IntPtr.Zero);
        return achada;
    }
    public static string OrdemDasJanelas(uint alvo) {
        var ordem = new StringBuilder();
        EnumWindows((janela, dado) => {
            uint processo;
            GetWindowThreadProcessId(janela, out processo);
            if (processo != alvo || !IsWindowVisible(janela)) return true;
            var nome = new StringBuilder(64);
            GetClassName(janela, nome, 64);
            ordem.Append(nome.ToString()).Append(" > ");
            return true;
        }, IntPtr.Zero);
        return ordem.ToString();
    }
    public static IntPtr Encontrar(uint alvo, string titulo) {
        IntPtr achada = IntPtr.Zero;
        EnumWindows((janela, dado) => {
            uint processo;
            GetWindowThreadProcessId(janela, out processo);
            if (processo != alvo) return true;
            var texto = new StringBuilder(64);
            GetWindowText(janela, texto, 64);
            if (texto.ToString() != titulo) return true;
            achada = janela;
            return false;
        }, IntPtr.Zero);
        return achada;
    }
}
'@

function Clicar([int]$X, [int]$Y) {
    $posicao = [IntPtr](($Y -shl 16) -bor $X)
    [void][Win]::PostMessage($script:janela, 0x0200, [IntPtr]::Zero, $posicao)
    Start-Sleep -Milliseconds 120
    [void][Win]::PostMessage($script:janela, 0x0201, [IntPtr]1, $posicao)
    Start-Sleep -Milliseconds 60
    [void][Win]::PostMessage($script:janela, 0x0202, [IntPtr]::Zero, $posicao)
    Start-Sleep -Milliseconds 500
}
function Digitar([string]$Texto) {
    foreach ($caractere in $Texto.ToCharArray()) {
        [void][Win]::PostMessage($script:janela, 0x0102, [IntPtr][int]$caractere, [IntPtr]1)
        Start-Sleep -Milliseconds 40
    }
    Start-Sleep -Milliseconds 400
}
function Fotografar([string]$Nome) {
    $area = New-Object Win+RECT
    [void][Win]::GetWindowRect($script:janela, [ref]$area)
    $imagem = New-Object System.Drawing.Bitmap(($area.Right - $area.Left), ($area.Bottom - $area.Top))
    $grafico = [System.Drawing.Graphics]::FromImage($imagem)
    $hdc = $grafico.GetHdc()
    [void][Win]::PrintWindow($script:janela, $hdc, 2)
    $grafico.ReleaseHdc($hdc)
    $grafico.Dispose()
    $imagem.Save((Join-Path $Saida "$Nome.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    $imagem.Dispose()
}
function ContarClipes { @(Get-ChildItem $clipes -Filter *.mp4 | Where-Object { $_.Name -notmatch 'parcial' }).Count }

$interface = Start-Process -FilePath (Join-Path $raiz $Executavel) -WindowStyle Hidden -PassThru
Start-Sleep -Seconds 3
$script:janela = [Win]::Encontrar([uint32]$interface.Id, 'ZAGA')
"janela: $($script:janela)"
if ($script:janela -eq [IntPtr]::Zero) { throw 'janela da interface nao encontrada' }
[void][Win]::MoveWindow($script:janela, -6000, -6000, 1136, 749, $false)
[void][Win]::ShowWindow($script:janela, 4)
Start-Sleep -Seconds 2

Fotografar 'inicio'
Clicar 100 148
Start-Sleep -Seconds 3
Fotografar 'clipes'
"clipes no inicio: $(ContarClipes)"
Clicar 980 56
Fotografar 'clipes_ordem'
Clicar 500 650
Clicar 830 56
Digitar 'zaga'
Fotografar 'clipes_busca'
foreach ($tecla in 1..4) {
    [void][Win]::PostMessage($script:janela, 0x0100, [IntPtr]0x08, [IntPtr]1)
    [void][Win]::PostMessage($script:janela, 0x0101, [IntPtr]0x08, [IntPtr]0xC0000001)
    Start-Sleep -Milliseconds 60
}
Start-Sleep -Milliseconds 400
Clicar 100 192
Fotografar 'config_captura'
Clicar 990 250
Fotografar 'config_seletor'
Clicar 500 680
Clicar 565 118
Fotografar 'config_desempenho'
Clicar 780 118
Fotografar 'config_geral'
Clicar 990 118
Fotografar 'config_atalhos'

Clicar 100 148
Start-Sleep -Milliseconds 500
Clicar 380 200
Start-Sleep -Seconds 4
Fotografar 'editor'

$larguraTira = 858
$xEsquerda = 244
$yTira = 645
$inicioArrasto = [IntPtr](($yTira -shl 16) -bor $xEsquerda)
$fimArrasto = [IntPtr](($yTira -shl 16) -bor ($xEsquerda + [int]($larguraTira / 2)))
[void][Win]::PostMessage($script:janela, 0x0200, [IntPtr]::Zero, $inicioArrasto)
Start-Sleep -Milliseconds 150
[void][Win]::PostMessage($script:janela, 0x0201, [IntPtr]1, $inicioArrasto)
Start-Sleep -Milliseconds 150
[void][Win]::PostMessage($script:janela, 0x0200, [IntPtr]1, $fimArrasto)
Start-Sleep -Milliseconds 150
[void][Win]::PostMessage($script:janela, 0x0202, [IntPtr]::Zero, $fimArrasto)
Start-Sleep -Seconds 1
Fotografar 'editor_corte'
Clicar 1000 702
Start-Sleep -Seconds 3
Fotografar 'editor_salvo'
"clipes depois de salvar como novo: $(ContarClipes)"

if ($MostrarVideo) {
    Add-Type -AssemblyName System.Windows.Forms
    $secundario = [System.Windows.Forms.Screen]::AllScreens | Where-Object { -not $_.Primary } | Select-Object -First 1
    if ($secundario) {
        $x = $secundario.Bounds.X + 5
        $y = $secundario.Bounds.Y + 200
        [void][Win]::MoveWindow($script:janela, $x, $y, 1060, 740, $true)
        Start-Sleep -Milliseconds 500
        $video = [Win]::EncontrarPorClasse([uint32]$interface.Id, 'ZagaVideo')
        [void][Win]::SetWindowPos($script:janela, [IntPtr](-1), 0, 0, 0, 0, 0x0013)
        [void][Win]::SetWindowPos($video, [IntPtr](-1), 0, 0, 0, 0, 0x0013)
        foreach ($rodada in 1..3) {
            Start-Sleep -Seconds 1
            $imagem = New-Object System.Drawing.Bitmap(1060, 740)
            $grafico = [System.Drawing.Graphics]::FromImage($imagem)
            $grafico.CopyFromScreen($x, $y, 0, 0, (New-Object System.Drawing.Size(1060, 740)))
            $grafico.Dispose()
            $corBarra = $imagem.GetPixel(100, 400)
            $naoPretos = 0
            $amostras = 0
            for ($px = 260; $px -lt 1010; $px += 25) {
                for ($py = 100; $py -lt 530; $py += 25) {
                    $cor = $imagem.GetPixel($px, $py)
                    $amostras++
                    if (($cor.R + $cor.G + $cor.B) -gt 30) { $naoPretos++ }
                }
            }
            $imagem.Dispose()
            $barraOk = ($corBarra.R -eq 16) -and ($corBarra.G -eq 14) -and ($corBarra.B -eq 24)
            "rodada ${rodada}: barra visivel=$barraOk, video=$naoPretos/$amostras, ordem=$([Win]::OrdemDasJanelas([uint32]$interface.Id))"
        }
    }
}

[void][Win]::MoveWindow($script:janela, -6000, -6000, 1136, 749, $true)
Start-Sleep -Milliseconds 500
Clicar 404 55
Digitar ' final'
[void][Win]::PostMessage($script:janela, 0x0100, [IntPtr]0x0D, [IntPtr]1)
[void][Win]::PostMessage($script:janela, 0x0101, [IntPtr]0x0D, [IntPtr]0xC0000001)
Start-Sleep -Seconds 3
Fotografar 'editor_renomeado'
"renomeado: " + (Test-Path (Join-Path $clipes 'Zaga insana 1v4 final.mp4'))

Clicar 938 55
Start-Sleep -Seconds 2
Fotografar 'apos_excluir'
"excluido da pasta: " + (-not (Test-Path (Join-Path $clipes 'Zaga insana 1v4 final.mp4')))

"encerrar: " + (& (Join-Path $raiz 'scripts\pedido_motor.ps1') '"Encerrar"')
Start-Sleep -Seconds 2
if (-not $interface.HasExited) { Stop-Process -Id $interface.Id -Force -Confirm:$false }
"telas: $((Get-ChildItem $Saida -Filter *.png).Name -join ', ')"
"arquivos: "
Get-ChildItem $clipes -Filter *.mp4 | ForEach-Object { "$($_.Name) $([int]($_.Length / 1KB)) KB" }
