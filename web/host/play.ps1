# Terraria in the browser on this PC: starts the page's server (web/host/terraria_host.py) and the multiplayer bridge
# (tools/ws-bridge/ws_bridge.py) without windows, then opens the default browser at the game's address.
#   play.ps1 [start|stop|status] [-NoBrowser]      (play-in-browser.cmd calls it; -NoBrowser: tests)
# Both listen on 127.0.0.1 only. Their process ids are kept in work\play-in-browser.pids; "stop" ends exactly those.
param([string]$Action = 'start', [switch]$NoBrowser)
$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)   # VAEL - TERRARIA
$Port = 7790          # the page:            http://terraria.localhost:7790/
$BridgePort = 7780    # Join via IP:          ws://127.0.0.1:7780 (the page's default bridge)
$HostTcpPort = 7777   # Host & Play, joiners: 127.0.0.1:7777 (the game's default port)
$Url = "http://terraria.localhost:$Port/"
$PidFile = Join-Path $Root 'work\play-in-browser.pids'
$LogDir = Join-Path $Root 'work\logs'
$Install = if ($env:TERRARIA_DIR) { $env:TERRARIA_DIR } else { 'C:\Users\user\Documents\Games\Terraria' }

function Listening($p) {
  return [bool](Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort $p -State Listen -ErrorAction SilentlyContinue)
}
function Find-Python {
  # pythonw: no console window at all
  $c = Get-Command pythonw.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($c) { return $c.Source }
  return (Get-Command python.exe -CommandType Application | Select-Object -First 1).Source
}
function Saved {
  if (Test-Path $PidFile) { return @(Get-Content $PidFile | Where-Object { $_ -match '^\d+$' } | ForEach-Object { [int]$_ }) }
  return @()
}

switch ($Action) {
  'stop' {
    $n = 0
    foreach ($id in Saved) {
      $p = Get-Process -Id $id -ErrorAction SilentlyContinue
      if ($p -and $p.ProcessName -like 'python*') { Stop-Process -Id $id -Force; $n++ }
    }
    if (Test-Path $PidFile) { Remove-Item $PidFile -Force }   # our own pid list
    Write-Output "Stopped $n process(es). (Saves stay in the browser.)"
    exit 0
  }
  'status' {
    Write-Output ("page server on 127.0.0.1:{0}: {1}" -f $Port, $(if (Listening $Port) { 'running' } else { 'not running' }))
    Write-Output ("bridge on 127.0.0.1:{0}: {1}" -f $BridgePort, $(if (Listening $BridgePort) { 'running' } else { 'not running' }))
    exit 0
  }
}

if (-not (Test-Path (Join-Path $Install 'Content'))) {
  Write-Output "No Terraria install at $Install (set TERRARIA_DIR to the folder that holds Terraria.exe and Content)."
  exit 1
}
New-Item -ItemType Directory -Force $LogDir | Out-Null
$py = Find-Python
$started = @()
if (-not (Listening $Port)) {
  $a = @("`"$Root\web\host\terraria_host.py`"", '--port', $Port, '--quiet', '--install', "`"$Install`"", '--log', "`"$LogDir\terraria-host.log`"")
  $started += (Start-Process -FilePath $py -ArgumentList $a -WindowStyle Hidden -PassThru).Id
}
if (-not (Listening $BridgePort)) {
  $a = @("`"$Root\tools\ws-bridge\ws_bridge.py`"", '--port', $BridgePort, '--origin', "http://terraria.localhost:$Port", '--listen', $HostTcpPort, '--to-page', '--log', "`"$LogDir\ws-bridge-play.log`"")
  $started += (Start-Process -FilePath $py -ArgumentList $a -WindowStyle Hidden -PassThru).Id
}
if ($started.Count) { Add-Content -Path $PidFile -Value $started }
# the server is up when the page answers (the first start after a game update also makes its caches: a few seconds)
$deadline = (Get-Date).AddSeconds(180)
while ((Get-Date) -lt $deadline) {
  try {
    $r = Invoke-WebRequest -Uri "http://127.0.0.1:$Port/" -UseBasicParsing -TimeoutSec 5
    if ($r.StatusCode -eq 200) { break }
  } catch { Start-Sleep -Milliseconds 300 }
}
if (-not $NoBrowser) { Start-Process $Url }
Write-Output "Terraria: $Url  (stop: play-in-browser.cmd stop)"
