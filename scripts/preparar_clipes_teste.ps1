param([Parameter(Mandatory)][string]$Pasta)

$validador = Join-Path $env:TEMP 'zaga_ffmpeg_validador'
New-Item -ItemType Directory -Force $validador, $Pasta | Out-Null
$ffmpeg = Get-ChildItem $validador -Recurse -Filter ffmpeg.exe -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $ffmpeg) {
    $ProgressPreference = 'SilentlyContinue'
    $zip = Join-Path $validador 'ffmpeg.zip'
    $url = 'https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip'
    Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
    Expand-Archive $zip -DestinationPath $validador -Force
    Remove-Item $zip -Force -Confirm:$false
    $ffmpeg = Get-ChildItem $validador -Recurse -Filter ffmpeg.exe | Select-Object -First 1
}

$clipes = @(
    @{ Nome = 'VALORANT 2026-11-06 21-37-12'; Segundos = 30; Fonte = 'testsrc2=size=1280x720:rate=60' }
    @{ Nome = 'Counter-Strike 2 2026-11-06 20-11-40'; Segundos = 15; Fonte = 'smptebars=size=1280x720:rate=60' }
    @{ Nome = 'Marvel Rivals 2026-01-14 22-16-05'; Segundos = 20; Fonte = 'life=size=1280x720:rate=60:mold=10:ratio=0.3:death_color=#301060:life_color=#9a66ff' }
    @{ Nome = 'Zaga insana 1v4'; Segundos = 20; Fonte = 'testsrc2=size=1280x720:rate=60' }
)
foreach ($clipe in $clipes) {
    $destino = Join-Path $Pasta "$($clipe.Nome).mp4"
    & $ffmpeg.FullName -hide_banner -v error -y `
        -f lavfi -i $clipe.Fonte -f lavfi -i "anullsrc=channel_layout=stereo:sample_rate=48000" `
        -t $clipe.Segundos -c:v libx264 -preset veryfast -pix_fmt yuv420p -g 120 -bf 0 `
        -c:a aac -b:a 160k -shortest $destino
    "gerado: $($clipe.Nome) ($((Get-Item $destino).Length / 1MB -as [int]) MB)"
}
