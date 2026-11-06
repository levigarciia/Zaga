param([Parameter(Mandatory)][string]$Origem)

$raiz = Split-Path -Parent $PSScriptRoot
$marca = Join-Path $raiz 'assets\marca'
$icones = Join-Path $raiz 'assets\icones'
New-Item -ItemType Directory -Force $marca, $icones | Out-Null

Add-Type -AssemblyName System.Drawing

function Redimensionar([string]$Entrada, [string]$Saida, [int]$Largura) {
    $imagem = [System.Drawing.Image]::FromFile($Entrada)
    $altura = [int]($imagem.Height * $Largura / $imagem.Width)
    $destino = New-Object System.Drawing.Bitmap($Largura, $altura, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $grafico = [System.Drawing.Graphics]::FromImage($destino)
    $grafico.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $grafico.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $grafico.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $grafico.DrawImage($imagem, 0, 0, $Largura, $altura)
    $grafico.Dispose()
    $destino.Save($Saida, [System.Drawing.Imaging.ImageFormat]::Png)
    $destino.Dispose()
    $imagem.Dispose()
}

Redimensionar (Join-Path $Origem 'ZAGA-logo-claro.png') (Join-Path $marca 'logo.png') 480
Redimensionar (Join-Path $Origem 'ZAGA-simbolo.png') (Join-Path $marca 'simbolo.png') 720
Copy-Item (Join-Path $Origem 'ZAGA.ico') (Join-Path $marca 'zaga.ico') -Force

$tracos = @{
    casa            = '<path d="M3 11l9-8 9 8"/><path d="M5 9.5V21h5v-6h4v6h5V9.5"/>'
    clipes          = '<rect x="3" y="4" width="18" height="16" rx="3"/><path d="M10 9l5 3-5 3z"/>'
    engrenagem      = '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.9 2.9l-.1-.1a1.7 1.7 0 0 0-2.8 1.2V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-2.9-1.2l-.1.1a2 2 0 1 1-2.8-2.9l.1-.1a1.7 1.7 0 0 0-1.2-2.8H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.2-2.9l-.1-.1a2 2 0 1 1 2.9-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 2.9 1.2l.1-.1a2 2 0 1 1 2.8 2.9l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>'
    relogio         = '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>'
    monitor         = '<rect x="3" y="4" width="18" height="13" rx="2"/><path d="M8 21h8M12 17v4"/>'
    folha           = '<path d="M5 19c0-9 6-14 15-15 0 10-4 16-12 16-1.5 0-3-.3-3-1z"/><path d="M4 21c2-5 5-8 10-11"/>'
    busca           = '<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/>'
    filtro          = '<path d="M4 5h16l-6.5 7.5V19l-3 1.5v-8z"/>'
    mais            = '<circle cx="12" cy="5" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="12" cy="19" r="1.2"/>'
    lixeira         = '<path d="M4 7h16M9 7V4h6v3M6 7l1 14h10l1-14M10 11v6M14 11v6"/>'
    compartilhar    = '<path d="M12 15V4M8 8l4-4 4 4"/><path d="M5 13v6a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-6"/>'
    tesoura         = '<circle cx="6" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M8.2 8.2L20 20M8.2 15.8L20 4"/>'
    voltar          = '<path d="M19 12H5M11 6l-6 6 6 6"/>'
    lapis           = '<path d="M4 20l1-4L16 5l3 3L8 19z"/><path d="M14 7l3 3"/>'
    tocar           = '<path d="M7 4l13 8-13 8z" fill="white"/>'
    pausar          = '<rect x="6" y="4" width="4" height="16" rx="1" fill="white"/><rect x="14" y="4" width="4" height="16" rx="1" fill="white"/>'
    volume          = '<path d="M4 9v6h4l5 4V5L8 9z"/><path d="M16.5 8.5a5 5 0 0 1 0 7M19 6a8.5 8.5 0 0 1 0 12"/>'
    mudo            = '<path d="M4 9v6h4l5 4V5L8 9z"/><path d="M17 9l5 6M22 9l-5 6"/>'
    minimizar       = '<path d="M6 12h12"/>'
    maximizar       = '<rect x="6" y="6" width="12" height="12" rx="1"/>'
    fechar          = '<path d="M6 6l12 12M18 6L6 18"/>'
    divisa          = '<path d="M6 9l6 6 6-6"/>'
    onda            = '<path d="M3 12h2M7 8v8M11 4v16M15 8v8M19 11v2M21 12h0"/>'
    teclado         = '<rect x="2" y="6" width="20" height="13" rx="2"/><path d="M6 10h0M10 10h0M14 10h0M18 10h0M7 15h10"/>'
    disco           = '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 14h18M7 17h0M11 17h0"/>'
    qualidade       = '<path d="M12 3l2.7 5.6 6.1.9-4.4 4.3 1 6.1-5.4-2.9-5.4 2.9 1-6.1-4.4-4.3 6.1-.9z"/>'
    chip            = '<rect x="6" y="6" width="12" height="12" rx="2"/><path d="M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4"/>'
    alto_falante    = '<path d="M4 9v6h4l5 4V5L8 9z"/><path d="M17 9a4 4 0 0 1 0 6"/>'
    microfone       = '<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3M9 21h6"/>'
    cursor          ='<path d="M5 3l14 8-6 2-2 6z"/>'
    inicializar     = '<path d="M12 3v9"/><path d="M6.3 7.3a8 8 0 1 0 11.4 0"/>'
    pasta           = '<path d="M3 7a2 2 0 0 1 2-2h4l2 3h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>'
    confirmar       = '<path d="M5 13l4 4L19 7"/>'
}

$semBom = New-Object System.Text.UTF8Encoding($false)
foreach ($nome in $tracos.Keys) {
    $conteudo = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">' + $tracos[$nome] + '</svg>'
    [IO.File]::WriteAllText((Join-Path $icones "$nome.svg"), $conteudo, $semBom)
}
"marca: $((Get-ChildItem $marca).Name -join ', ')"
"icones: $((Get-ChildItem $icones).Count)"
