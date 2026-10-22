@echo off
rem Builds tools\refdump_worldgen\bin\RefDumpX87Ext.exe (.NET Framework 4, x86): the second harness program (more maths tables, raw x87
rem instruction results, the second set of float probes with a dump of the JIT's machine code). RefDumpWorldgen.exe is not rebuilt.
setlocal
set HERE=%~dp0
for %%I in ("%HERE%..\..") do set ROOT=%%~fI
if not exist "%HERE%bin" mkdir "%HERE%bin"
"C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe" /nologo /platform:x86 /target:exe /optimize+ /nowarn:1718 /r:"%ROOT%\work\refserver\Microsoft.Xna.Framework.dll" /out:"%HERE%bin\RefDumpX87Ext.exe" "%HERE%ExtProgram.cs" "%HERE%ExtX87.cs" "%HERE%ExtProbes.cs" "%HERE%ExtProbesGen.cs"
exit /b %ERRORLEVEL%
