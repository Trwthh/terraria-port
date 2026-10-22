@echo off
rem Builds the XNA stand-in (tools\refdump_draw\bin\xna\Microsoft.Xna.Framework.Graphics.dll / .Game.dll) and the reference
rem harness tools\refdump_draw\bin\RefDumpDraw.exe. The stand-in has the identity of XNA 4.0 (name, version, public key token
rem 842cf8be1de50553): delay-signed with XNA's public key, then marked signed (fakesign.py) so the CLR's strong-name bypass
rem loads it. Its public surface is generated from the real assemblies in work\refserver (gen\XnaStubGen.cs); members the
rem game needs are hand-written in xna\Graphics, xna\Game (//@impl markers). .NET Framework 4 csc (C# 5), x86.
setlocal
set HERE=%~dp0
for %%I in ("%HERE%..\..") do set ROOT=%%~fI
set REAL=%ROOT%\work\refserver
set CSC=C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe
if not exist "%HERE%bin\xna" mkdir "%HERE%bin\xna"
if not exist "%HERE%gen\out" mkdir "%HERE%gen\out"
"%CSC%" /nologo /out:"%HERE%gen\XnaStubGen.exe" "%HERE%gen\XnaStubGen.cs" || exit /b 1
"%HERE%gen\XnaStubGen.exe" --pubkey "%REAL%\Microsoft.Xna.Framework.Graphics.dll" "%HERE%gen\out\xna_public.snk" || exit /b 1
"%HERE%gen\XnaStubGen.exe" "%REAL%\Microsoft.Xna.Framework.Graphics.dll" "%HERE%gen\out\Graphics.g.cs" "%HERE%xna\Graphics" || exit /b 1
"%HERE%gen\XnaStubGen.exe" "%REAL%\Microsoft.Xna.Framework.Game.dll" "%HERE%gen\out\Game.g.cs" "%HERE%xna\Game" || exit /b 1
"%CSC%" /nologo /nowarn:0067,0108,0109,0114,0169,0414,0649,0659,0660,0661,0618,0628 /target:library /unsafe /optimize+ /delaysign+ /keyfile:"%HERE%gen\out\xna_public.snk" /r:"%REAL%\Microsoft.Xna.Framework.dll" /out:"%HERE%bin\xna\Microsoft.Xna.Framework.Graphics.dll" "%HERE%gen\out\Graphics.g.cs" "%HERE%xna\StandIn.cs" "%HERE%xna\Graphics\*.cs" || exit /b 1
python "%HERE%fakesign.py" "%HERE%bin\xna\Microsoft.Xna.Framework.Graphics.dll" || exit /b 1
"%CSC%" /nologo /nowarn:0067,0108,0109,0114,0169,0414,0649,0659,0660,0661,0618,0628 /target:library /unsafe /optimize+ /delaysign+ /keyfile:"%HERE%gen\out\xna_public.snk" /r:"%REAL%\Microsoft.Xna.Framework.dll" /r:"%HERE%bin\xna\Microsoft.Xna.Framework.Graphics.dll" /r:System.Windows.Forms.dll /r:System.Drawing.dll /out:"%HERE%bin\xna\Microsoft.Xna.Framework.Game.dll" "%HERE%gen\out\Game.g.cs" "%HERE%xna\StandIn.cs" "%HERE%xna\Game\*.cs" || exit /b 1
python "%HERE%fakesign.py" "%HERE%bin\xna\Microsoft.Xna.Framework.Game.dll" || exit /b 1
if exist "%HERE%RefDumpDraw.cs" (
"%CSC%" /nologo /platform:x86 /target:exe /optimize+ /unsafe /r:"%HERE%bin\xna\Microsoft.Xna.Framework.Graphics.dll" /r:"%HERE%bin\xna\Microsoft.Xna.Framework.Game.dll" /r:"%REAL%\Microsoft.Xna.Framework.dll" /r:System.Windows.Forms.dll /r:System.Drawing.dll /r:System.Numerics.dll /out:"%HERE%bin\RefDumpDraw.exe" "%HERE%RefDumpDraw.cs" "%HERE%Gallery.cs" "%HERE%Interact.cs" "%HERE%Cover.cs" "%HERE%Probe*.cs" "%HERE%NetClient.cs" "%HERE%..\refdump_net\NetVirt.cs" "%HERE%AudioRec.cs" "%HERE%Detour.cs" "%HERE%..\refdump_ticks\Common.cs" || exit /b 1
)
if exist "%HERE%DrawJit.cs" (
"%CSC%" /nologo /platform:x86 /target:exe /optimize+ /unsafe /main:DrawJitProgram /r:"%HERE%bin\xna\Microsoft.Xna.Framework.Graphics.dll" /r:"%HERE%bin\xna\Microsoft.Xna.Framework.Game.dll" /r:"%REAL%\Microsoft.Xna.Framework.dll" /r:System.Windows.Forms.dll /r:System.Drawing.dll /r:System.Numerics.dll /out:"%HERE%bin\RefJitDraw.exe" "%HERE%RefDumpDraw.cs" "%HERE%Gallery.cs" "%HERE%Interact.cs" "%HERE%Cover.cs" "%HERE%Probe*.cs" "%HERE%NetClient.cs" "%HERE%..\refdump_net\NetVirt.cs" "%HERE%AudioRec.cs" "%HERE%Detour.cs" "%HERE%DrawJit.cs" "%HERE%..\refdump_ticks\Common.cs" "%HERE%..\refdump_worldgen\JitDump.cs" || exit /b 1
)
exit /b 0
