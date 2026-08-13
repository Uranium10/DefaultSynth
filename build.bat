@echo off
setlocal
rem Bundle DefaultSynth as .clap and .vst3.
rem
rem Rust rewrites its incremental objects constantly. On machines where the
rem workspace lives under a watched folder (OneDrive/Desktop indexers, AV
rem real-time scanning), those files get briefly locked and the build dies with
rem "os error 32". Keeping the cache off the workspace avoids it. Override
rem CARGO_TARGET_DIR before running this file if you want it somewhere else.
if not defined CARGO_TARGET_DIR set "CARGO_TARGET_DIR=C:\tmp\defaultsynth-target"

set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where cargo.exe >nul 2>&1
if errorlevel 1 (
  echo [DefaultSynth] Cargo was not found. Install Rust with rustup and retry.
  exit /b 1
)

cargo xtask bundle defaultsynth --release %*
set "EXIT_CODE=%errorlevel%"
if "%EXIT_CODE%"=="0" echo [DefaultSynth] Bundles are in "%CARGO_TARGET_DIR%\bundled".
exit /b %EXIT_CODE%
