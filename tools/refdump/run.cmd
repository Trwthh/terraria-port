@echo off
rem Builds and runs the harness headlessly. Writes refdata\*.json and refdata\MANIFEST.json.
rem Saves go to work\refdump-save; the game folder is never written. Time-boxed by an in-process watchdog (280 s).
setlocal
set HERE=%~dp0
call "%HERE%build.cmd" || exit /b 1
for %%I in ("%HERE%..\..") do set ROOT=%%~fI
"%HERE%bin\RefDump.exe" "%ROOT%"
exit /b %ERRORLEVEL%
