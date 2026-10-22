@echo off
rem Builds and runs the world-generation harness headlessly (no window, no socket). Writes refdata\worldgen\*.
rem   1. gen   : the real game generates ref_small (seed 12345, small, classic) on a thread-pool thread like the dedicated server, with a callback after every pass;
rem              the world is saved to work\worldgen-scratch\main\Worlds\ref_small.wld (never into the game folder). About 2 minutes.
rem   2. x87   : System.Math reference tables      -> refdata\worldgen\x87
rem   3. probes: float expression probes           -> refdata\worldgen\float_probes
rem Determinism experiments (each is a fresh process, optional): see refdata\worldgen\README.md section "Re-running".
setlocal
set HERE=%~dp0
call "%HERE%build.cmd" || exit /b 1
for %%I in ("%HERE%..\..") do set ROOT=%%~fI
"%HERE%bin\RefDumpWorldgen.exe" gen "%ROOT%\refdata\worldgen" main passes task || exit /b 1
"%HERE%bin\RefDumpWorldgen.exe" x87 "%ROOT%\refdata\worldgen\x87" || exit /b 1
"%HERE%bin\RefDumpWorldgen.exe" floatprobe "%ROOT%\refdata\worldgen\float_probes" || exit /b 1
exit /b 0
