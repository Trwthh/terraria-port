@echo off
rem Builds tools\refdump_worldgen\bin\RefDumpCollisionJit.exe (.NET Framework 4, x86): tools\refdump_collision's three sources,
rem unchanged, plus Common.cs, JitDump.cs and CollisionJit.cs (entry point CollisionJitProgram). See CollisionJit.cs.
setlocal
set HERE=%~dp0
set COLL=%HERE%..\refdump_collision\
if not exist "%HERE%bin" mkdir "%HERE%bin"
"C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe" /nologo /platform:x86 /target:exe /optimize+ /main:CollisionJitProgram /out:"%HERE%bin\RefDumpCollisionJit.exe" "%COLL%RefDumpCollision.cs" "%COLL%Stamps.cs" "%COLL%Cases.cs" "%HERE%Common.cs" "%HERE%JitDump.cs" "%HERE%CollisionJit.cs"
exit /b %ERRORLEVEL%
