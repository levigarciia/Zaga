param(
    [string]$Executavel = 'target\distribuicao\release\ZAGA.exe'
)

$ErrorActionPreference = 'Stop'
$raiz = Split-Path -Parent $PSScriptRoot
$origem = (Resolve-Path -LiteralPath (Join-Path $raiz $Executavel)).Path
$pastaTeste = Join-Path $env:TEMP ('zaga_atualizador_' + [guid]::NewGuid().ToString('N'))
$pastaAlvo = Join-Path $pastaTeste 'aplicativo portatil'
$preparacao = Join-Path $pastaTeste 'preparacao'
$guardiao = $null
$auxiliar = $null
$variaveis = @{}
foreach ($nome in @('ZAGA_DADOS', 'ZAGA_PIPE')) {
    $variaveis[$nome] = [Environment]::GetEnvironmentVariable($nome)
}

function Iniciar-Aplicador([string]$Binario, [string]$Manifesto) {
    Start-Process -FilePath $Binario -ArgumentList @('--aplicar-atualizacao', ('"' + $Manifesto + '"')) `
        -WindowStyle Hidden -PassThru
}

function Escrever-Plano([string]$Caminho, [hashtable]$Plano) {
    $json = ConvertTo-Json -InputObject $Plano -Compress
    [IO.File]::WriteAllText($Caminho, $json, (New-Object Text.UTF8Encoding($false)))
}

try {
    New-Item -ItemType Directory -Path $pastaAlvo, $preparacao -Force | Out-Null
    $env:ZAGA_DADOS = Join-Path $pastaTeste 'dados'
    $env:ZAGA_PIPE = 'atualizador_' + [guid]::NewGuid().ToString('N')
    $alvo = Join-Path $pastaAlvo 'ZAGA.exe'
    $nova = Join-Path $preparacao 'nova.exe'
    $aplicador = Join-Path $preparacao 'aplicador.exe'
    $manifesto = Join-Path $preparacao 'plano.json'
    Copy-Item -LiteralPath $origem -Destination $alvo
    $gravador = [IO.File]::Open($alvo, [IO.FileMode]::Append)
    try { $gravador.WriteByte(1) } finally { $gravador.Dispose() }
    $hashAnterior = (Get-FileHash -LiteralPath $alvo -Algorithm SHA256).Hash
    Copy-Item -LiteralPath $origem -Destination $nova
    Copy-Item -LiteralPath $origem -Destination $aplicador
    $hashNovo = (Get-FileHash -LiteralPath $nova -Algorithm SHA256).Hash.ToLowerInvariant()
    $guardiao = Start-Process powershell.exe -ArgumentList @(
        '-NoProfile', '-NonInteractive', '-Command', 'Start-Sleep -Seconds 3'
    ) -WindowStyle Hidden -PassThru
    $plano = @{
        alvo = $alvo; arquivo = $nova; tamanho = (Get-Item -LiteralPath $nova).Length
        sha256 = $hashNovo; pid = $guardiao.Id; reiniciar = $false
    }
    Escrever-Plano $manifesto $plano
    $auxiliar = Iniciar-Aplicador $aplicador $manifesto
    Start-Sleep -Milliseconds 200
    if ((Get-FileHash -LiteralPath $alvo -Algorithm SHA256).Hash -ne $hashAnterior) {
        throw 'O aplicador modificou o executável antes da saída do processo'
    }
    if (-not $auxiliar.WaitForExit(30000)) { throw 'O aplicador não encerrou a tempo' }
    if ($auxiliar.ExitCode -ne 0) { throw 'O aplicador retornou falha' }
    if ((Get-FileHash -LiteralPath $alvo -Algorithm SHA256).Hash.ToLowerInvariant() -ne $hashNovo) {
        throw 'O executável atualizado não corresponde ao download validado'
    }
    [IO.File]::WriteAllText($nova, 'arquivo adulterado')
    $plano.pid = 0
    Escrever-Plano $manifesto $plano
    $auxiliar = Iniciar-Aplicador $aplicador $manifesto
    if (-not $auxiliar.WaitForExit(30000)) { throw 'O aplicador adulterado não encerrou a tempo' }
    if ($auxiliar.ExitCode -eq 0) { throw 'O aplicador aceitou um pacote adulterado' }
    if ((Get-FileHash -LiteralPath $alvo -Algorithm SHA256).Hash.ToLowerInvariant() -ne $hashNovo) {
        throw 'A falha alterou o executável válido'
    }
    if (@(Get-ChildItem -LiteralPath $pastaAlvo -Force).Count -ne 1) {
        throw 'O aplicador deixou substitutos ou backups após concluir'
    }
    Write-Output 'Atualizador validado: espera pelo processo, substituição e recusa de arquivo adulterado.'
} finally {
    foreach ($nome in $variaveis.Keys) {
        [Environment]::SetEnvironmentVariable($nome, $variaveis[$nome])
    }
    foreach ($processo in @($auxiliar, $guardiao)) {
        if ($null -ne $processo -and -not $processo.HasExited) { $processo.Kill(); $processo.WaitForExit() }
    }
    $destinoLimpeza = [IO.Path]::GetFullPath($pastaTeste)
    $limiteLimpeza = [IO.Path]::GetFullPath($env:TEMP).TrimEnd('\') + '\'
    if (-not $destinoLimpeza.StartsWith($limiteLimpeza, [StringComparison]::OrdinalIgnoreCase) `
        -or -not ([IO.Path]::GetFileName($destinoLimpeza).StartsWith('zaga_atualizador_'))) {
        throw 'Pasta de teste fora do limite de limpeza'
    }
    if (Test-Path -LiteralPath $destinoLimpeza) { Remove-Item -LiteralPath $destinoLimpeza -Recurse -Force }
}
