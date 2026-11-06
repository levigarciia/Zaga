param(
    [int]$CapturaSegundos = 40,
    [int]$ClipeSegundos = 30,
    [string]$ConfigJson = ''
)

$raiz = Split-Path -Parent $PSScriptRoot
$dados = Join-Path $env:TEMP 'zaga_teste_e2e'
if (Test-Path $dados) { Remove-Item -Recurse -Force $dados -Confirm:$false }
New-Item -ItemType Directory -Force $dados | Out-Null
if ($ConfigJson) { $ConfigJson | Out-File -Encoding ascii (Join-Path $dados 'config.json') }

$env:ZAGA_DADOS = $dados
$argumentosMovimento = @(
    '-NoProfile', '-ExecutionPolicy', 'Bypass',
    '-File', (Join-Path $raiz 'scripts\gerador_movimento.ps1'),
    '-Segundos', ($CapturaSegundos + 15)
)
$movimento = Start-Process powershell -ArgumentList $argumentosMovimento -WindowStyle Hidden -PassThru
$motor = Start-Process -FilePath (Join-Path $raiz 'target\release\zaga.exe') -ArgumentList '--bandeja' -WindowStyle Hidden -PassThru
$pedir = Join-Path $raiz 'scripts\pedido_motor.ps1'

Start-Sleep -Seconds 5
"estado inicial: " + (& $pedir '"ObterEstado"')

$cpuInicial = (Get-Process -Id $motor.Id).TotalProcessorTime
$inicio = Get-Date
Start-Sleep -Seconds ([Math]::Max(1, $CapturaSegundos - 5))

$processo = Get-Process -Id $motor.Id
$decorrido = ((Get-Date) - $inicio).TotalSeconds
$cpu = ($processo.TotalProcessorTime - $cpuInicial).TotalSeconds / $decorrido * 100
"CPU: {0:N2}% de um nucleo ({1:N2}% do total) | privada {2:N1} MiB | working set {3:N1} MiB | threads {4}" -f `
    $cpu, ($cpu / [Environment]::ProcessorCount), ($processo.PrivateMemorySize64 / 1MB), ($processo.WorkingSet64 / 1MB), $processo.Threads.Count

$gpu = Get-Counter "\GPU Engine(pid_$($motor.Id)*)\Utilization Percentage" -ErrorAction SilentlyContinue
if ($gpu) {
    $gpu.CounterSamples | Where-Object { $_.CookedValue -gt 0.01 } | ForEach-Object {
        "GPU {0}: {1:N2}%" -f ($_.InstanceName -replace '.*engtype_', ''), $_.CookedValue
    }
}
$vram = Get-Counter "\GPU Process Memory(pid_$($motor.Id)*)\Dedicated Usage" -ErrorAction SilentlyContinue
if ($vram) {
    $vram.CounterSamples | Where-Object { $_.CookedValue -gt 0 } | ForEach-Object { "VRAM: {0:N1} MiB" -f ($_.CookedValue / 1MB) }
}

"estado: " + (& $pedir '"ObterEstado"')
$anel = Join-Path $dados 'anel_*.bin'
Get-ChildItem $anel -ErrorAction SilentlyContinue | ForEach-Object { "anel: {0:N0} MiB" -f ($_.Length / 1MB) }
"salvar: " + (& $pedir ('{"SalvarClipe":{"segundos":' + $ClipeSegundos + '}}'))
$cronometro = [Diagnostics.Stopwatch]::StartNew()
$padraoClipe = Join-Path $dados 'clipes\*.mp4'
while ($cronometro.Elapsed.TotalSeconds -lt 60) {
    $concluidos = @(Get-ChildItem $padraoClipe -ErrorAction SilentlyContinue | Where-Object { $_.Name -notmatch '\.parcial\.mp4$' })
    if ($concluidos.Count -gt 0) { break }
    Start-Sleep -Milliseconds 100
}
"tempo para salvar: {0:N2} s" -f $cronometro.Elapsed.TotalSeconds
$amostraCpu = (Get-Process -Id $motor.Id).TotalProcessorTime
Start-Sleep -Seconds 1

"[log]"
Get-Content (Join-Path $dados 'motor.log')
"[clipe]"
$clipe = Get-ChildItem (Join-Path $dados 'clipes\*.mp4') -ErrorAction SilentlyContinue | Select-Object -First 1
if ($clipe) {
    $clipe.Name
    Copy-Item $clipe.FullName (Join-Path $env:TEMP 'zaga_ultimo_clipe.mp4') -Force
    & (Join-Path $raiz 'target\debug\examples\verificar_clipe.exe') $clipe.FullName
} else {
    'nenhum clipe gerado'
}

"encerrar: " + (& $pedir '"Encerrar"')
Start-Sleep -Seconds 2
if ($motor.HasExited) { "motor encerrou: $($motor.ExitCode)" } else { 'motor nao encerrou'; Stop-Process -Id $motor.Id -Force -Confirm:$false }
"anel apagado: " + (-not (Test-Path $anel))
if (-not $movimento.HasExited) { Stop-Process -Id $movimento.Id -Force -Confirm:$false }
foreach ($tentativa in 1..10) {
    try {
        Remove-Item -Recurse -Force $dados -Confirm:$false -ErrorAction Stop
        break
    } catch {
        Start-Sleep -Milliseconds 500
    }
}
