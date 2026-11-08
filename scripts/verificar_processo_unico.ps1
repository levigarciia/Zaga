param(
    [string]$Executavel = 'target\release\ZAGA.exe',
    [switch]$IniciarNaBandeja
)

$ErrorActionPreference = 'Stop'
$raiz = Split-Path -Parent $PSScriptRoot
$dados = Join-Path $env:TEMP ('zaga_processo_' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $dados | Out-Null
'{"captura_ativa":false}' | Set-Content -Encoding ascii (Join-Path $dados 'config.json')
$anteriores = @{}
foreach ($nome in @('ZAGA_DADOS', 'ZAGA_PIPE', 'ZAGA_TESTE_SEM_FOCO')) {
    $anteriores[$nome] = [Environment]::GetEnvironmentVariable($nome)
}
$env:ZAGA_DADOS = $dados
$env:ZAGA_PIPE = 'zaga_processo_' + [guid]::NewGuid().ToString('N')
$env:ZAGA_TESTE_SEM_FOCO = '1'

Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class JanelasZaga {
    public delegate bool Visitante(IntPtr janela, IntPtr dado);
    [DllImport("user32.dll")] public static extern bool EnumWindows(Visitante visitante, IntPtr dado);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr janela, out uint processo);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr janela, StringBuilder texto, int tamanho);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr janela, StringBuilder classe, int tamanho);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindow(string classe, string titulo);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr janela, uint mensagem, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr janela);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr janela, int x, int y, int largura, int altura, bool redesenhar);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr janela, int comando);
    public static IntPtr Principal(uint alvo) {
        IntPtr achada = IntPtr.Zero;
        EnumWindows((janela, dado) => {
            uint processo;
            GetWindowThreadProcessId(janela, out processo);
            var texto = new StringBuilder(256);
            GetWindowText(janela, texto, 256);
            if (processo != alvo || texto.ToString() != "ZAGA") return true;
            var classe = new StringBuilder(256);
            GetClassName(janela, classe, 256);
            if (classe.ToString().StartsWith("ZagaMotorJanela")) return true;
            achada = janela;
            return false;
        }, IntPtr.Zero);
        return achada;
    }
}
'@

$processo = $null
try {
    $opcoes = @{ FilePath = (Join-Path $raiz $Executavel); WindowStyle = 'Hidden'; PassThru = $true }
    if ($IniciarNaBandeja) { $opcoes.ArgumentList = '--bandeja' }
    $processo = Start-Process @opcoes
    Start-Sleep -Seconds 3
    if ($processo.HasExited) { throw "Aplicativo encerrou: $($processo.ExitCode)" }
    $principal = [JanelasZaga]::Principal($processo.Id)
    if ($IniciarNaBandeja) {
        if ($principal -ne [IntPtr]::Zero) { throw 'Iniciar na bandeja criou a interface antecipadamente' }
        $bandeja = [JanelasZaga]::FindWindow(('ZagaMotorJanela_' + $env:ZAGA_PIPE), 'ZAGA')
        [void][JanelasZaga]::PostMessage($bandeja, 0x8004, [IntPtr]::Zero, [IntPtr]::Zero)
        Start-Sleep -Seconds 1
        $principal = [JanelasZaga]::Principal($processo.Id)
    }
    if ($principal -eq [IntPtr]::Zero) { throw 'Janela principal ausente' }
    [void][JanelasZaga]::MoveWindow($principal, -6000, -6000, 1136, 749, $false)
    [void][JanelasZaga]::ShowWindow($principal, 4)
    if (-not [JanelasZaga]::IsWindowVisible($principal)) { throw 'Janela de teste não abriu fora da tela' }
    $estado = & (Join-Path $PSScriptRoot 'pedido_motor.ps1') '"ObterEstado"'
    if (-not ($estado | ConvertFrom-Json).Estado) { throw 'Estado do motor indisponível' }
    [void][JanelasZaga]::PostMessage($principal, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
    Start-Sleep -Seconds 1
    if ($processo.HasExited) { throw 'Fechar a janela encerrou o motor' }
    if ([JanelasZaga]::IsWindowVisible($principal)) { throw 'Janela não foi ocultada na bandeja' }
    $bandeja = [JanelasZaga]::FindWindow(('ZagaMotorJanela_' + $env:ZAGA_PIPE), 'ZAGA')
    [void][JanelasZaga]::PostMessage($bandeja, 0x8004, [IntPtr]::Zero, [IntPtr]::Zero)
    Start-Sleep -Seconds 1
    if ([JanelasZaga]::Principal($processo.Id) -ne $principal) { throw 'Janela foi recriada desnecessariamente' }
    if (-not [JanelasZaga]::IsWindowVisible($principal)) { throw 'Bandeja não restaurou a janela' }
    $segunda = Start-Process -FilePath (Join-Path $raiz $Executavel) -WindowStyle Hidden -PassThru
    if (-not $segunda.WaitForExit(5000)) { throw 'Segunda instância continuou aberta' }
    if ($segunda.ExitCode -ne 0) { throw 'Segunda instância retornou erro' }
    $filhos = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($processo.Id)")
    if ($filhos.Count -ne 0) { throw 'Aplicativo criou processos filhos' }
    & (Join-Path $PSScriptRoot 'pedido_motor.ps1') '"Encerrar"'
    if (-not $processo.WaitForExit(10000)) { throw 'Aplicativo não encerrou' }
    if ($processo.ExitCode -ne 0) { throw "Saída inesperada: $($processo.ExitCode)" }
    'Validado: fechar preserva a bandeja, janela reutilizada, segunda instância termina, sem processos filhos.'
} finally {
    if ($processo -and -not $processo.HasExited) { Stop-Process -Id $processo.Id -Force }
    foreach ($nome in $anteriores.Keys) {
        [Environment]::SetEnvironmentVariable($nome, $anteriores[$nome])
    }
    "Dados de diagnóstico: $dados"
}
