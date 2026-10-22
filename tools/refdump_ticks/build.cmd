@echo off
rem Builds tools\refdump_ticks\bin\RefDumpTicks.exe (.NET Framework 4, x86). Pure reflection: no reference to the game. Common.cs is a copy of tools\refdump_worldgen\Common.cs.
setlocal
set HERE=%~dp0
if not exist "%HERE%bin" mkdir "%HERE%bin"
"C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe" /nologo /platform:x86 /target:exe /optimize+ /out:"%HERE%bin\RefDumpTicks.exe" "%HERE%Ticks.cs" "%HERE%Common.cs"
exit /b %ERRORLEVEL%
