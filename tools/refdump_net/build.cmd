@echo off
rem Builds tools\refdump_net\bin\RefDumpNet.exe (the net run's server, NetServer.cs) and RefJitNet.exe (its JIT-dumping
rem twin, NetJit.cs). .NET Framework 4 csc, x86 (the game's 32-bit JIT). The game is used by reflection; the real XNA
rem Framework of work\refserver is referenced for its Vector2. The client side is tools\refdump_draw (RefDumpDraw.exe net).
setlocal
set HERE=%~dp0
for %%I in ("%HERE%..\..") do set ROOT=%%~fI
set CSC="C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe" /nologo /platform:x86 /target:exe /optimize+ /unsafe /r:"%ROOT%\work\refserver\Microsoft.Xna.Framework.dll" /r:System.Core.dll
if not exist "%HERE%bin" mkdir "%HERE%bin"
%CSC% /main:WG /out:"%HERE%bin\RefDumpNet.exe" "%HERE%NetServer.cs" "%HERE%NetVirt.cs" "%HERE%..\refdump_draw\Detour.cs" "%HERE%..\refdump_ticks\Common.cs" || exit /b 1
%CSC% /main:NetJitProgram /out:"%HERE%bin\RefJitNet.exe" "%HERE%NetServer.cs" "%HERE%NetJit.cs" "%HERE%NetVirt.cs" "%HERE%..\refdump_draw\Detour.cs" "%HERE%..\refdump_ticks\Common.cs" "%HERE%..\refdump_worldgen\JitDump.cs" || exit /b 1
python "%HERE%..\refdump_worldgen\set_laa.py" "%HERE%bin\RefDumpNet.exe" || exit /b 1
python "%HERE%..\refdump_worldgen\set_laa.py" "%HERE%bin\RefJitNet.exe" || exit /b 1
exit /b 0
