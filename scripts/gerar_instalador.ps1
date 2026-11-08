param(
    [string]$Compilador = 'ISCC.exe',
    [string]$Versao = '',
    [string]$PastaCompilacao = 'target\distribuicao'
)

$ErrorActionPreference = 'Stop'
$raiz = Split-Path -Parent $PSScriptRoot
Push-Location $raiz
try {
    $manifesto = Get-Content -LiteralPath (Join-Path $raiz 'Cargo.toml') -Encoding UTF8 -Raw
    $versaoManifesto = [regex]::Match($manifesto, '(?m)^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"').Groups[1].Value
    if (-not $versaoManifesto) { throw 'Versão não encontrada no manifesto' }
    if (-not $Versao) { $Versao = $versaoManifesto }
    if ($Versao -ne $versaoManifesto) { throw 'A versão da distribuição difere da versão do aplicativo' }
    $targetDistribuicao = Join-Path $raiz $PastaCompilacao
    $executavel = Join-Path $targetDistribuicao 'release\ZAGA.exe'
    & cargo build --release --locked -p zaga_interface --target-dir $targetDistribuicao
    if ($LASTEXITCODE -ne 0) { throw 'Falha ao compilar o Zaga' }
    & $Compilador "/DVersao=$Versao" "/DBinario=$executavel" (Join-Path $raiz 'installer\zaga.iss')
    if ($LASTEXITCODE -ne 0) { throw 'Falha ao gerar o instalador' }
    $pasta = Join-Path $raiz 'dist'
    $atualizacao = Join-Path $pasta "ZAGA-$Versao-x64.exe"
    Copy-Item -LiteralPath $executavel -Destination $atualizacao -Force
    $portatil = Join-Path $pasta "Zaga-$Versao-portatil-x64.zip"
    $temporario = Join-Path $pasta ('portatil_' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path (Join-Path $temporario 'licenses') -Force | Out-Null
    try {
        Copy-Item -LiteralPath $executavel -Destination $temporario
        Copy-Item -LiteralPath (Join-Path $raiz 'docs\licenses\FortniteReplayDecompressor.txt') `
            -Destination (Join-Path $temporario 'licenses')
        Copy-Item -LiteralPath (Join-Path $raiz 'assets\fonts\Inter.OFL.txt'), `
            (Join-Path $raiz 'assets\fonts\SpaceGrotesk.OFL.txt') -Destination (Join-Path $temporario 'licenses')
        Compress-Archive -LiteralPath (Join-Path $temporario 'ZAGA.exe'), (Join-Path $temporario 'licenses') `
            -DestinationPath $portatil -Force
    } finally {
        $destinoLimpeza = [IO.Path]::GetFullPath($temporario)
        $limiteLimpeza = [IO.Path]::GetFullPath($pasta) + [IO.Path]::DirectorySeparatorChar
        if (-not $destinoLimpeza.StartsWith($limiteLimpeza, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Pasta temporária fora da distribuição'
        }
        Remove-Item -LiteralPath $destinoLimpeza -Recurse -Force
    }
    $arquivos = @(Get-Item -LiteralPath $portatil, $atualizacao, (Join-Path $pasta "ZagaSetup-$Versao-x64.exe"))
    $somas = $arquivos | ForEach-Object {
        $hash = Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256
        '{0}  {1}' -f $hash.Hash.ToLowerInvariant(), $_.Name
    }
    $somas | Set-Content -Encoding ascii (Join-Path $pasta 'SHA256SUMS.txt')
} finally {
    Pop-Location
}
