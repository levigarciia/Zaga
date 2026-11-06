param([Parameter(Mandatory)][string]$Clipe)

$pasta = Join-Path $env:TEMP 'zaga_ffmpeg_validador'
$ffmpeg = (Get-ChildItem $pasta -Recurse -Filter ffmpeg.exe | Select-Object -First 1).FullName
$ffprobe = (Get-ChildItem $pasta -Recurse -Filter ffprobe.exe | Select-Object -First 1).FullName
if (-not $ffmpeg) { throw "FFmpeg portatil nao encontrado em $pasta" }

$campos = 'stream=codec_name,profile,level,width,height,pix_fmt,color_range,color_space,avg_frame_rate,nb_frames,start_time,duration'
& $ffprobe -v error -show_entries $campos -of compact=p=0 $Clipe

$erros = & $ffmpeg -hide_banner -v error -i $Clipe -fps_mode passthrough -f null - 2>&1 | ForEach-Object { "$_" }
$errosReais = @($erros | Where-Object { $_ -and $_ -notmatch 'non monotonically increasing dts to muxer' })
"erros de decodificacao: $($errosReais.Count)"
$errosReais | Select-Object -First 5

$intervalos = & $ffprobe -v error -select_streams v -show_entries packet=pts_time -of csv=p=0 $Clipe |
    ForEach-Object { [double]($_ -replace ',', '') }
$deltas = for ($indice = 1; $indice -lt $intervalos.Count; $indice++) { ($intervalos[$indice] - $intervalos[$indice - 1]) * 1000 }
$estatistica = $deltas | Measure-Object -Average -Maximum -Minimum
"intervalo entre quadros: media {0:N2} ms, min {1:N2} ms, max {2:N2} ms" -f $estatistica.Average, $estatistica.Minimum, $estatistica.Maximum

& $ffmpeg -hide_banner -nostats -i $Clipe -af volumedetect -vn -f null - 2>&1 |
    ForEach-Object { "$_" } | Select-String 'max_volume' | ForEach-Object { ($_ -replace '.*max_volume', 'audio max_volume') }
