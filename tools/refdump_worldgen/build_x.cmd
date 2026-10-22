@echo off
rem Builds tools\refdump_worldgen\bin\RefDumpWorldgenX.exe (.NET Framework 4, x86): the parameterised world generation harness
rem (ProgramX.cs, GenX.cs, JitDump.cs + the shared Common.cs). RefDumpWorldgen.exe and its sources are not touched.
rem The game is used by reflection only. /platform:x86: the same 32-bit JIT as the game. The executable is marked
rem large-address-aware afterwards (set_laa.py), as TerrariaServer.exe is: a large world needs more than 2 GB.
setlocal
set HERE=%~dp0
if not exist "%HERE%bin" mkdir "%HERE%bin"
"C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe" /nologo /platform:x86 /target:exe /optimize+ /out:"%HERE%bin\RefDumpWorldgenX.exe" "%HERE%ProgramX.cs" "%HERE%Common.cs" "%HERE%GenX.cs" "%HERE%JitDump.cs" || exit /b 1
python "%HERE%set_laa.py" "%HERE%bin\RefDumpWorldgenX.exe" || exit /b 1
exit /b 0
