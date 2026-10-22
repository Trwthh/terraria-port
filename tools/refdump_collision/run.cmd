@echo off
rem Builds and runs the collision harness headlessly (hidden console child, in-process watchdog 560 s).
rem Writes refdata\collision\*.bin and collision.json. Saves go to work\refdump-save and work\collision-scratch; the game folder is never written.
rem Optional: set REFDUMP_COLLISION_ONLY=Family1;Family2  and  REFDUMP_COLLISION_COUNT=n  for a quick run.
setlocal
set HERE=%~dp0
call "%HERE%build.cmd" || exit /b 1
"%HERE%bin\RefDumpCollision.exe" %*
exit /b %ERRORLEVEL%
