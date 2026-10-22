@echo off
rem Builds tools\refdump_collision\bin\RefDumpCollision.exe (.NET Framework 4, x86). Pure reflection: no reference to the game.
setlocal
set HERE=%~dp0
if not exist "%HERE%bin" mkdir "%HERE%bin"
"C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe" /nologo /platform:x86 /target:exe /optimize+ /out:"%HERE%bin\RefDumpCollision.exe" "%HERE%RefDumpCollision.cs" "%HERE%Stamps.cs" "%HERE%Cases.cs"
exit /b %ERRORLEVEL%
