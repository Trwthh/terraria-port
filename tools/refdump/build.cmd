@echo off
rem Builds tools\refdump\bin\RefDump.exe (.NET Framework 4, x86). Pure reflection: no reference to the game.
setlocal
set HERE=%~dp0
if not exist "%HERE%bin" mkdir "%HERE%bin"
"C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe" /nologo /platform:x86 /target:exe /optimize+ /out:"%HERE%bin\RefDump.exe" "%HERE%RefDump.cs" "%HERE%RefDumpFramed.cs"
exit /b %ERRORLEVEL%
