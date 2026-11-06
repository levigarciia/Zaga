param(
    [string]$Compilador = 'ISCC.exe',
    [string]$Versao = '0.1.0'
)

$ErrorActionPreference = 'Stop'
$raiz = Split-Path -Parent $PSScriptRoot
Push-Location $raiz
try {
    & cargo build --release --locked -p zaga_interface
    if ($LASTEXITCODE -ne 0) { throw 'Falha ao compilar o Zaga' }
    & $Compilador "/DVersao=$Versao" (Join-Path $raiz 'installer\zaga.iss')
    if ($LASTEXITCODE -ne 0) { throw 'Falha ao gerar o instalador' }
    $pasta = Join-Path $raiz 'dist'
    $portatil = Join-Path $pasta "Zaga-$Versao-portatil-x64.zip"
    Compress-Archive -LiteralPath (Join-Path $raiz 'target\release\zaga.exe') -DestinationPath $portatil -Force
    $arquivos = @(Get-ChildItem -LiteralPath $pasta -File | Where-Object Extension -In '.exe', '.zip')
    $somas = $arquivos | ForEach-Object {
        $hash = Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256
        '{0}  {1}' -f $hash.Hash.ToLowerInvariant(), $_.Name
    }
    $somas | Set-Content -Encoding ascii (Join-Path $pasta 'SHA256SUMS.txt')
} finally {
    Pop-Location
}
