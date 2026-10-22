@echo off
rem Builds and runs the lighting reference harness headlessly. Writes refdata\light\*.bin and refdata\light\MANIFEST.txt.
rem Reads work\refsave\Worlds\ref_small.wld through a scratch copy in work\light-scratch; the game folder is never written.
rem Time-boxed by an in-process watchdog (240 s).
setlocal
set HERE=%~dp0
call "%HERE%build.cmd" || exit /b 1
for %%I in ("%HERE%..\..") do set ROOT=%%~fI
"%HERE%bin\RefDumpLight.exe" "%ROOT%\work\refsave\Worlds\ref_small.wld" "%ROOT%\refdata\light"
exit /b %ERRORLEVEL%
