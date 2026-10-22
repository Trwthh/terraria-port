@echo off
rem Builds tools\refdump_worldgen\bin\RefDumpLightJit.exe (.NET Framework 4, x86): tools\refdump_light\RefDumpLight.cs,
rem unchanged, plus Common.cs, JitDump.cs and LightJit.cs (entry point LightJitProgram). See LightJit.cs.
setlocal
set HERE=%~dp0
set LIGHT=%HERE%..\refdump_light\
if not exist "%HERE%bin" mkdir "%HERE%bin"
"C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe" /nologo /platform:x86 /target:exe /optimize+ /main:LightJitProgram /out:"%HERE%bin\RefDumpLightJit.exe" "%LIGHT%RefDumpLight.cs" "%HERE%Common.cs" "%HERE%JitDump.cs" "%HERE%LightJit.cs"
exit /b %ERRORLEVEL%
