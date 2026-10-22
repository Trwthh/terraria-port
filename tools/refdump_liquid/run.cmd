@echo off
rem Builds and runs the liquid reference harness headlessly, one process per scenario (each reloads the world from a scratch copy in
rem work\liquid-scratch; the game folder is never written). Writes refdata\liquid\settle.*, settle_trace.* and <scenario>.bin.
rem Every process is time-boxed by an in-process watchdog (240 s).
setlocal
set HERE=%~dp0
call "%HERE%build.cmd" || exit /b 1
for %%I in ("%HERE%..\..") do set ROOT=%%~fI
for %%S in (settle_trace lake_drain surface_fill shimmer_drain honey_drain lava_open) do (
  "%HERE%bin\RefDumpLiquid.exe" "%ROOT%\work\refsave\Worlds\ref_small.wld" "%ROOT%\refdata\liquid" %%S || exit /b 1
)
exit /b 0
