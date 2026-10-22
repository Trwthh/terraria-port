@echo off
rem Builds FmtProbe with the .NET Framework 4 compiler (x86, the game's runtime) and writes fmt_probe.out.
set HERE=%~dp0
"C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe" /nologo /platform:x86 /target:exe /optimize+ /out:"%HERE%FmtProbe.exe" "%HERE%FmtProbe.cs" || exit /b 1
"%HERE%FmtProbe.exe" "%HERE%fmt_probe.out"
del "%HERE%FmtProbe.exe"
