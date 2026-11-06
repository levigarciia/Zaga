param([int]$Segundos = 60)
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$janela = New-Object System.Windows.Forms.Form
$janela.Text = 'Zaga: gerador de movimento para teste'
$janela.FormBorderStyle = 'FixedToolWindow'
$janela.StartPosition = 'Manual'
$area = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
$janela.Size = New-Object System.Drawing.Size(320, 180)
$janela.Location = New-Object System.Drawing.Point(($area.Right - 330), ($area.Bottom - 190))
$janela.TopMost = $true
$janela.ShowInTaskbar = $false
$relogio = New-Object System.Windows.Forms.Timer
$relogio.Interval = 15
$inicio = Get-Date
$relogio.Add_Tick({
    $tempo = ((Get-Date) - $inicio).TotalSeconds
    if ($tempo -ge $Segundos) { $janela.Close(); return }
    $vermelho = [int](127 + 127 * [Math]::Sin($tempo * 3))
    $verde = [int](127 + 127 * [Math]::Sin($tempo * 5))
    $janela.BackColor = [System.Drawing.Color]::FromArgb($vermelho, $verde, 80)
})
$relogio.Start()
[void]$janela.ShowDialog()
