@echo off
rem Builds tools\refdump_entities\bin\RefDumpEntities.exe (.NET Framework 4, x86). Pure reflection: no reference to the game.
rem Common.cs is tools\refdump_ticks\Common.cs (compiled in, not copied). The JIT-dumping twin is built by
rem tools\refdump_worldgen\build_refjit.cmd (RefJitEntities.exe = these sources + JitDump.cs + EntitiesJit.cs).
setlocal
set HERE=%~dp0
if not exist "%HERE%bin" mkdir "%HERE%bin"
"C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe" /nologo /platform:x86 /target:exe /optimize+ /out:"%HERE%bin\RefDumpEntities.exe" "%HERE%Entities.cs" "%HERE%ClientItems.cs" "%HERE%..\refdump_ticks\Common.cs"
exit /b %ERRORLEVEL%
