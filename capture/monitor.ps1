<#
 monitor.ps1 — "man in the middle" monitor for the FDAD-DCT radar app.

 It hooks the app's socket layer with Frida and records EVERY send/recv/connect/bind
 (TCP + UDP, both directions) to capture\radar-live.tsv, flushed live, so we can watch
 the real protocol in real time and validate it.

 Commands:
   .\monitor.ps1 -Mode start      # attach to the running app, (re)start capture
   .\monitor.ps1 -Mode stop       # detach
   .\monitor.ps1 -Mode status     # is it capturing? how many frames?
   .\monitor.ps1 -Mode tail       # show the last 25 captured ops
   .\monitor.ps1 -Mode decode     # decode radar-live.tsv with the RE'd protocol

 Typical session:
   .\monitor.ps1 -Mode start
   # ... connect the radar over Ethernet and operate the app ...
   .\monitor.ps1 -Mode decode
#>
param(
  [ValidateSet('start','stop','status','tail','decode')][string]$Mode = 'start',
  [string]$App  = 'FDAD-DCTv3.0.0_EN.exe',
  [int]   $TailLines = 25
)
$here  = Split-Path -Parent $MyInvocation.MyCommand.Path
$frida = Join-Path $env:APPDATA 'Python\Python312\Scripts\frida.exe'
$js    = Join-Path $here 'radar_socketdump.js'
$live  = Join-Path $here 'radar-live.tsv'
$out   = Join-Path $here 'radar-live.out'
$err   = Join-Path $here 'radar-live.err'
$pidf  = Join-Path $here '.frida.pid'

function Get-AppProc { Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Name -like ($App -replace '\.exe$','') + '*' } }

switch ($Mode) {
  'start' {
    if (-not (Test-Path $frida)) { $frida = (Get-Command frida -ErrorAction SilentlyContinue).Source }
    if (-not $frida) { Write-Error 'frida not found (pip install frida-tools)'; exit 1 }
    $proc = Get-AppProc
    if (-not $proc) { Write-Error "app '$App' is not running - start it first"; exit 1 }
    Get-Process frida -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Remove-Item $live, $out, $err -ErrorAction SilentlyContinue
    $p = Start-Process -FilePath $frida -ArgumentList '-n', $App, '-l', $js `
          -RedirectStandardOutput $out -RedirectStandardError $err -WindowStyle Hidden -PassThru
    $p.Id | Out-File $pidf -Encoding ascii
    Start-Sleep -Seconds 4
    Write-Host ("capture started (frida pid {0}) -> {1}" -f $p.Id, $live) -ForegroundColor Green
    Get-Content $err -ErrorAction SilentlyContinue | Select-String -Pattern 'hooks installed|Error|error' | ForEach-Object { $_.Line }
  }
  'stop' {
    if (Test-Path $pidf) { $id = (Get-Content $pidf) -as [int]; Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
    Get-Process frida -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Write-Host 'capture stopped' -ForegroundColor Green
  }
  'status' {
    $n = (Get-Process frida -ErrorAction SilentlyContinue | Measure-Object).Count
    $lines = if (Test-Path $live) { (Get-Content $live | Where-Object { $_ -match "`t" }).Count } else { 0 }
    Write-Host ("frida running: {0}   captured ops: {1}   file: {2}" -f $n, $lines, $live)
  }
  'tail' {
    if (Test-Path $live) { Get-Content $live -Tail $TailLines } else { Write-Host 'no capture file yet' }
  }
  'decode' {
    if (-not (Test-Path $live)) { Write-Error "no capture file ($live)"; exit 1 }
    python (Join-Path $here 'decode.py') $live
  }
}
