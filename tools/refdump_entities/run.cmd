@echo off
rem Builds and runs the entity reference scenarios headlessly (no window, no socket). Writes refdata\entities\npc.*,
rem proj.*, item.* (120 ticks per batch) and citem.* (client-side item use, 60 ticks per batch, ClientItems.cs). The world is a scratch copy of work\refsave\Worlds\ref_small.wld; nothing is written
rem to the game folder.
setlocal
set HERE=%~dp0
call "%HERE%build.cmd" || exit /b 1
for %%I in ("%HERE%..\..") do set ROOT=%%~fI
"%HERE%bin\RefDumpEntities.exe" npc "%ROOT%\refdata\entities" 120 || exit /b 1
"%HERE%bin\RefDumpEntities.exe" proj "%ROOT%\refdata\entities" 120 || exit /b 1
"%HERE%bin\RefDumpEntities.exe" item "%ROOT%\refdata\entities" 120 || exit /b 1
"%HERE%bin\RefDumpEntities.exe" citem "%ROOT%\refdata\entities" 60 || exit /b 1
exit /b 0
