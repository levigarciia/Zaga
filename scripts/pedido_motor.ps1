param(
    [Parameter(Mandatory)][string]$Pedido,
    [int]$EsperaMs = 3000,
    [string]$NomePipe = $(if ($env:ZAGA_PIPE) { $env:ZAGA_PIPE } else { 'zaga_motor' })
)
$pipe = New-Object System.IO.Pipes.NamedPipeClientStream('.', $NomePipe, [System.IO.Pipes.PipeDirection]::InOut)
$pipe.Connect($EsperaMs)
$utf8 = New-Object System.Text.UTF8Encoding($false)
$escritor = New-Object System.IO.StreamWriter($pipe, $utf8)
$escritor.AutoFlush = $true
$leitor = New-Object System.IO.StreamReader($pipe, $utf8)
$escritor.WriteLine($Pedido)
$leitor.ReadLine()
$pipe.Dispose()
