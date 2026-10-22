@echo off
rem Builds the reference-run JIT dumpers (.NET Framework 4, x86) into tools\refdump_worldgen\bin: each is a reference
rem harness's own sources, unchanged, plus Common.cs / JitDump.cs and a wrapper entry point (RefJit.cs, TicksJit.cs,
rem WorldgenJit.cs, WorldgenXJit.cs, EntitiesJit.cs) that runs the harness and then dumps the code of every game method that ran.
rem Collision and lighting: build_colljit.cmd, build_lightjit.cmd. Driver: run_refjit.sh.
setlocal
set HERE=%~dp0
for %%I in ("%HERE%..\..") do set ROOT=%%~fI
set CSC="C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe" /nologo /platform:x86 /target:exe /optimize+
if not exist "%HERE%bin" mkdir "%HERE%bin"
%CSC% /main:RefJitProgram /out:"%HERE%bin\RefJitP1.exe" "%HERE%..\refdump\RefDump.cs" "%HERE%..\refdump\RefDumpFramed.cs" "%HERE%Common.cs" "%HERE%JitDump.cs" "%HERE%RefJit.cs" || exit /b 1
%CSC% /main:RefJitProgram /out:"%HERE%bin\RefJitLiquid.exe" "%HERE%..\refdump_liquid\RefDumpLiquid.cs" "%HERE%Common.cs" "%HERE%JitDump.cs" "%HERE%RefJit.cs" || exit /b 1
%CSC% /main:RefJitProgram /out:"%HERE%bin\RefJitSky.exe" "%HERE%..\refdump_sky\RefDumpSky.cs" "%HERE%Common.cs" "%HERE%JitDump.cs" "%HERE%RefJit.cs" || exit /b 1
%CSC% /main:TicksJitProgram /out:"%HERE%bin\RefJitTicks.exe" "%HERE%..\refdump_ticks\Ticks.cs" "%HERE%..\refdump_ticks\Common.cs" "%HERE%JitDump.cs" "%HERE%TicksJit.cs" || exit /b 1
%CSC% /main:EntitiesJitProgram /out:"%HERE%bin\RefJitEntities.exe" "%HERE%..\refdump_entities\Entities.cs" "%HERE%..\refdump_entities\ClientItems.cs" "%HERE%..\refdump_ticks\Common.cs" "%HERE%JitDump.cs" "%HERE%EntitiesJit.cs" || exit /b 1
%CSC% /main:WorldgenJitProgram /r:"%ROOT%\work\refserver\Microsoft.Xna.Framework.dll" /out:"%HERE%bin\RefJitWorldgen.exe" "%HERE%Common.cs" "%HERE%Gen.cs" "%HERE%X87.cs" "%HERE%FloatProbe.cs" "%HERE%JitDump.cs" "%HERE%WorldgenJit.cs" || exit /b 1
%CSC% /main:WorldgenXJitProgram /out:"%HERE%bin\RefJitWorldgenX.exe" "%HERE%Common.cs" "%HERE%GenX.cs" "%HERE%JitDump.cs" "%HERE%WorldgenXJit.cs" || exit /b 1
python "%HERE%set_laa.py" "%HERE%bin\RefJitWorldgenX.exe" || exit /b 1
call "%HERE%build_colljit.cmd" || exit /b 1
call "%HERE%build_lightjit.cmd" || exit /b 1
exit /b 0
