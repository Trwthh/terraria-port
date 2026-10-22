@echo off
rem Builds and runs the second harness program (RefDumpX87Ext.exe): raw x87 instruction results, dense and additional System.Math tables,
rem the second set of float probes with the machine code the JIT produced. Headless, under a minute, writes only below refdata\worldgen.
rem x87raw reads the argument lists of refdata\worldgen\x87\*.bin (run.cmd writes those).
rem The generated probe source (ExtProbesGen.cs) comes from float_model\gen_cs.py; it is checked in, python is not needed to run this.
setlocal
set HERE=%~dp0
call "%HERE%build_ext.cmd" || exit /b 1
for %%I in ("%HERE%..\..") do set ROOT=%%~fI
set OUT=%ROOT%\refdata\worldgen
"%HERE%bin\RefDumpX87Ext.exe" x87raw "%OUT%\x87_raw" "%OUT%\x87" || exit /b 1
"%HERE%bin\RefDumpX87Ext.exe" x87dense "%OUT%\x87_dense" || exit /b 1
"%HERE%bin\RefDumpX87Ext.exe" x87more "%OUT%\x87_more" || exit /b 1
"%HERE%bin\RefDumpX87Ext.exe" floatprobe2 "%OUT%\float_probes2" || exit /b 1
exit /b 0
