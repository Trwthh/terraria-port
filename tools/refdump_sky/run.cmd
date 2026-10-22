@echo off
rem Builds and runs the sky colour reference harness headlessly. Writes refdata\sky\sky_colors.csv.
rem Reads work\refsave\Worlds\ref_small.wld through a scratch copy in work\sky-scratch; the game folder is never written.
rem Time-boxed by an in-process watchdog (240 s).
setlocal
set HERE=%~dp0
call "%HERE%build.cmd" || exit /b 1
for %%I in ("%HERE%..\..") do set ROOT=%%~fI
"%HERE%bin\RefDumpSky.exe" "%ROOT%\work\refsave\Worlds\ref_small.wld" "%ROOT%\refdata\sky"
exit /b %ERRORLEVEL%
