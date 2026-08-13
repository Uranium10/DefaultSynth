@echo off
setlocal
rem Build and launch the standalone synth, no plugin host required.
rem
rem Pass extra arguments straight through, for example:
rem   run.bat --midi-input ""          list available MIDI inputs
rem   run.bat --midi-input "MPK mini"  play from a keyboard
rem   run.bat --output-device ""       list available output devices

if not defined CARGO_TARGET_DIR set "CARGO_TARGET_DIR=C:\tmp\defaultsynth-target"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

where cargo.exe >nul 2>&1
if errorlevel 1 (
  echo [DefaultSynth] Cargo was not found. Install Rust with rustup and retry.
  exit /b 1
)

cargo build --release --bin defaultsynth
if errorlevel 1 exit /b 1

rem WASAPI in shared mode hands the app a period size the device chooses, and
rem NIH-plug's standalone backend panics if it does not match what was asked for.
rem If it dies with "Received N samples, while the configured buffer size is ...",
rem set DEFAULTSYNTH_PERIOD to that N.
if not defined DEFAULTSYNTH_PERIOD set "DEFAULTSYNTH_PERIOD=1056"

echo [DefaultSynth] Starting with period size %DEFAULTSYNTH_PERIOD%.
"%CARGO_TARGET_DIR%\release\defaultsynth.exe" --backend wasapi --period-size %DEFAULTSYNTH_PERIOD% %*
exit /b %errorlevel%
