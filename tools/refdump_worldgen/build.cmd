@echo off
rem Builds tools\refdump_worldgen\bin\RefDumpWorldgen.exe (.NET Framework 4, x86). The game itself is used by reflection only; the one
rem compile-time reference is Microsoft.Xna.Framework.dll (read from the private server copy, never copied) for the Vector2 float probes.
rem /platform:x86 matters: the x87 table and the float probes must run in the same 32-bit JIT as the game.
setlocal
set HERE=%~dp0
for %%I in ("%HERE%..\..") do set ROOT=%%~fI
if not exist "%HERE%bin" mkdir "%HERE%bin"
"C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe" /nologo /platform:x86 /target:exe /optimize+ /r:"%ROOT%\work\refserver\Microsoft.Xna.Framework.dll" /out:"%HERE%bin\RefDumpWorldgen.exe" "%HERE%Program.cs" "%HERE%Common.cs" "%HERE%Gen.cs" "%HERE%X87.cs" "%HERE%FloatProbe.cs"
exit /b %ERRORLEVEL%
