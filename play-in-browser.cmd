@echo off
rem Terraria in the browser, on this PC only.
rem
rem   play-in-browser.cmd          start (if needed) and open http://terraria.localhost:7790/ in the default browser
rem   play-in-browser.cmd stop     end the page server and the multiplayer bridge this started
rem   play-in-browser.cmd status   are they running?
rem
rem - Needs Chrome or Edge 137+ (WebGPU). The game files are read from %TERRARIA_DIR% (default
rem   C:\Users\user\Documents\Games\Terraria), read-only; only Terraria.exe and Content are served.
rem - Starts, without windows: web\host\terraria_host.py on 127.0.0.1:7790 and tools\ws-bridge\ws_bridge.py on
rem   127.0.0.1:7780 (Join via IP; Host & Play joiners on 127.0.0.1:7777). Nothing listens beyond this PC.
rem - Characters, worlds and settings are kept in the browser (its private storage for terraria.localhost:7790).
rem - Logs: work\logs\terraria-host.log, work\logs\ws-bridge-play.log. Details: web\www\README.md.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0web\host\play.ps1" %1
