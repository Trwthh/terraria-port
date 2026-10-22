@echo off
rem Starts the machine-translated Terraria 1.4.5.8 client (port\crates\terraria-app, binary "terraria") in a normal,
rem visible window with the game's own audio volume.
rem
rem   run-terraria.cmd
rem
rem - game files:  %TERRARIA_DIR%  (default C:\Users\user\Documents\Games\Terraria). Read only: nothing is written there.
rem - saves:       work\port-save  below this folder (players, worlds, config.json, achievements.dat)
rem - if something goes wrong: the window title names it and work\port-save has
rem     port-crashlog.txt    what the port could not do (a missing piece: the game went on), with the last log lines
rem     client-crashlog.txt  the game's own crash log (an unhandled exception: the game shows it and ends)
rem     last-run.log         everything the client printed in the last run
rem - build first (Git Bash, from port\):
rem     CARGO_TARGET_DIR=target-app cargo build -p terraria-app --bin terraria --features client --profile client -j 4
rem   The optimised build (profile client-opt) is used instead when it exists.
rem - known problems: port\crates\terraria-app\README.md, section "Known issues"
setlocal
set "ROOT=%~dp0"
if not defined TERRARIA_DIR set "TERRARIA_DIR=C:\Users\user\Documents\Games\Terraria"
set "TERRARIA_SAVE_DIR=%ROOT%work\port-save"
set "TERRARIA_PROJECT_DIR=%ROOT%."

rem a visible, interactive run: none of the test switches
set "TERRARIA_HIDDEN="
set "TERRARIA_MUTE="
set "TERRARIA_VOLUME="
set "TERRARIA_INPUT_SCRIPT="
set "TERRARIA_FIXED_DT="
set "TERRARIA_SCREENSHOT="
set "TERRARIA_DISPLAY="
set "TERRARIA_KEEP_GOING="
set "TERRARIA_STRICT="
set "TERRARIA_GC_FULL="
set "TERRARIA_GC_VERIFY="
set "TERRARIA_LOAD_SLICE="
set "TERRARIA_AUDIO_PCM16="
set "TERRARIA_FRAME_TIMES=1"

set "EXE=%ROOT%port\target-app\client\terraria.exe"
if exist "%ROOT%port\target-app\client-opt\terraria.exe" set "EXE=%ROOT%port\target-app\client-opt\terraria.exe"
if not exist "%EXE%" (
  echo The client is not built: "%EXE%" is missing.
  echo Build it from port\ in Git Bash:
  echo   CARGO_TARGET_DIR=target-app cargo build -p terraria-app --bin terraria --features client --profile client -j 4
  exit /b 1
)
if not exist "%TERRARIA_DIR%\Content" (
  echo The game folder "%TERRARIA_DIR%" has no Content folder. Set TERRARIA_DIR to your Terraria install.
  exit /b 1
)
if not exist "%TERRARIA_SAVE_DIR%" mkdir "%TERRARIA_SAVE_DIR%"

echo Terraria (translated client): %EXE%
echo   game files %TERRARIA_DIR%
echo   saves      %TERRARIA_SAVE_DIR%
echo   log        %TERRARIA_SAVE_DIR%\last-run.log
"%EXE%" 2> "%TERRARIA_SAVE_DIR%\last-run.log"
echo The client ended with exit code %ERRORLEVEL%.
endlocal
