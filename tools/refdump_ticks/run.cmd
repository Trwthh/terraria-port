@echo off
rem Builds and runs the server-tick harness headlessly (no window, no socket). Writes refdata\ticks\world.* (3600 ticks, no player) and player.* (600 ticks, one scripted player).
rem The world is a scratch copy of work\refsave\Worlds\ref_small.wld; nothing is written to the game folder. About 1 minute per scenario.
setlocal
set HERE=%~dp0
call "%HERE%build.cmd" || exit /b 1
for %%I in ("%HERE%..\..") do set ROOT=%%~fI
"%HERE%bin\RefDumpTicks.exe" world "%ROOT%\refdata\ticks" 3600 || exit /b 1
"%HERE%bin\RefDumpTicks.exe" player "%ROOT%\refdata\ticks" 600 || exit /b 1
exit /b 0
