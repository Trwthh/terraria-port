@echo off
rem Builds tools\refdump_light\bin\RefDumpLight.exe (.NET Framework 4, x86). Pure reflection: no reference to the game.
setlocal
set HERE=%~dp0
if not exist "%HERE%bin" mkdir "%HERE%bin"
"C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe" /nologo /platform:x86 /target:exe /optimize+ /out:"%HERE%bin\RefDumpLight.exe" "%HERE%RefDumpLight.cs"
exit /b %ERRORLEVEL%
