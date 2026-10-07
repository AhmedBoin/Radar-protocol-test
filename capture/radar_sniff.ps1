<#
 radar_sniff.ps1 — capture the radar protocol (TCP + UDP) for the FDAD-DCT app.

 Two ways to capture (both write into this folder):

  1) FRIDA payload dump (recommended — gives the exact protocol bytes, no Wireshark):
       powershell -ExecutionPolicy Bypass -File radar_sniff.ps1 -Mode frida
     - Attaches to the running app and logs every socket send/recv to a .tsv file.
     - Then decode it:  -Mode decode

  2) RAW packet capture via built-in pktmon (needs an ELEVATED shell; -> .pcap):
       radar_sniff.ps1 -Mode start      # add filters for the radar ports, begin capture
       ... connect the radar over Ethernet and reproduce ...
       radar_sniff.ps1 -Mode stop
       radar_sniff.ps1 -Mode convert    # .etl -> .pcap (open in Wireshark)

 Examples:
   .\radar_sniff.ps1 -Mode frida
   .\radar_sniff.ps1 -Mode decode -Log .\radar-capture-20261007_120000.tsv
   .\radar_sniff.ps1 -Mode start  -RadarIP 192.168.8.167 -Port 5001
#>
param(
  [ValidateSet('frida','decode','start','stop','convert')]
  [string]$Mode = 'frida',
  [string]$App    = 'FDAD-DCTv3.0.0.exe',
  [string]$RadarIP = '192.168.8.167',
  [int]   $Port   = 5001,
  [int]   $Port2  = 5002,
  [string]$Log    = '',
  [string]$Etl    = ''
)

$here   = Split-Path -Parent $MyInvocation.MyCommand.Path
$frida  = Join-Path $env:APPDATA 'Python\Python312\Scripts\frida.exe'
$js     = Join-Path $here 'radar_socketdump.js'
$decode = Join-Path $here 'decode.py'

switch ($Mode) {

  'frida' {
    if (-not (Test-Path $frida)) { $frida = (Get-Command frida -ErrorAction SilentlyContinue).Source }
    if (-not $frida) { Write-Error 'frida not found (pip install frida-tools)'; exit 1 }
    if (-not (Get-Process $App.TrimEnd('.exe') -ErrorAction SilentlyContinue) -and
        -not (Get-Process ($App -replace '\.exe$','') -ErrorAction SilentlyContinue)) {
      Write-Host "WARNING: '$App' does not look like it is running. Start it first." -ForegroundColor Yellow
    }
    if (-not $Log) { $Log = Join-Path $here ("radar-capture-" + (Get-Date -Format 'yyyyMMdd_HHmmss') + ".tsv") }
    Write-Host "Attaching Frida to '$App'..." -ForegroundColor Cyan
    Write-Host "  logging socket payloads -> $Log" -ForegroundColor Green
    Write-Host "  (Ctrl+C to stop; then run:  .\radar_sniff.ps1 -Mode decode -Log <that file>)" -ForegroundColor Cyan
    & $frida -n $App -l $js *> $Log
  }

  'decode' {
    if (-not $Log) {
      $Log = Get-ChildItem $here -Filter 'radar-capture-*.tsv' -ErrorAction SilentlyContinue |
             Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName
    }
    if (-not $Log -or -not (Test-Path $Log)) { Write-Error 'no capture .tsv found'; exit 1 }
    Write-Host "Decoding $Log" -ForegroundColor Cyan
    python $decode $Log
  }

  'start' {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $pr = New-Object Security.Principal.WindowsPrincipal($id)
    if (-not $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
      Write-Host 'pktmon start needs an ELEVATED shell.' -ForegroundColor Yellow
    }
    if (-not $Etl) { $Etl = Join-Path $here ("radar-" + (Get-Date -Format 'yyyyMMdd_HHmmss') + ".etl") }
    pktmon filter remove
    pktmon filter add radar_tcp -p $Port  -t TCP
    pktmon filter add radar_udp -p $Port2 -t UDP
    pktmon start --capture --pkt-size 0 -f $Etl
    Write-Host "capturing radar traffic (file: $Etl)" -ForegroundColor Green
    Write-Host "  -> connect the radar (Ethernet), reproduce, then:  .\radar_sniff.ps1 -Mode stop" -ForegroundColor Cyan
    Write-Host "  NOTE: this captures UDP 5002 + TCP $Port. To catch the radar IP only, filter in Wireshark: ip.addr==$RadarIP"
  }

  'stop'    { pktmon stop; Write-Host 'capture stopped' -ForegroundColor Green }

  'convert' {
    if (-not $Etl) {
      $Etl = Get-ChildItem $here -Filter 'radar-*.etl' -ErrorAction SilentlyContinue |
             Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName
    }
    if (-not $Etl -or -not (Test-Path $Etl)) { Write-Error 'no .etl found'; exit 1 }
    $pcap = [IO.Path]::ChangeExtension($Etl, '.pcap')
    pktmon etl2pcap $Etl -o $pcap
    Write-Host "pcap -> $pcap  (open in Wireshark; filter: ip.addr==$RadarIP)" -ForegroundColor Green
  }
}
