@echo off
setlocal
title Building Sompio
cd /d "%~dp0"

where npm.cmd >nul 2>nul || (
  echo Node.js was not found. Install it from https://nodejs.org, restart your PC, then run this again.
  pause
  exit /b 1
)
where cargo >nul 2>nul || (
  echo Rust was not found. Install it from https://rustup.rs, restart your PC, then run this again.
  pause
  exit /b 1
)

rem Compiled pieces are kept outside this folder, so a new copy of the project only rebuilds what changed.
set "CARGO_TARGET_DIR=%LOCALAPPDATA%\Sompio Build"

echo Building Sompio. The first time takes 5 to 10 minutes; after that only what changed is rebuilt.
echo.

rem 1. The key maker, and the public half of your signing key for the app to trust.
echo Step 1 of 2: Sompio Key Maker
cd /d "%~dp0licence"
cargo build --release --bin sompio-keymaker || goto :fail
"%CARGO_TARGET_DIR%\release\sompio-keymaker.exe" init --public-out public.key >nul || goto :fail

rem 2. The app itself.
echo Step 2 of 2: Sompio
cd /d "%~dp0app"
call npm.cmd install --no-audit --no-fund || goto :fail
rem Old setup files go first, so only the one built now is handed out
del /q "%CARGO_TARGET_DIR%\release\bundle\nsis\*-setup.exe" >nul 2>nul
rem A plain local build: nothing here talks to GitHub. Updates are only switched on in published releases.
call npm.cmd run tauri build || goto :fail

cd /d "%~dp0"
if not exist "Sompio for Windows" mkdir "Sompio for Windows"
del /q "Sompio for Windows\*" >nul 2>nul
copy /y "%CARGO_TARGET_DIR%\release\sompio.exe" "Sompio for Windows\Sompio.exe" >nul
copy /y "%CARGO_TARGET_DIR%\release\sompio-keymaker.exe" "Sompio for Windows\Sompio Key Maker.exe" >nul
for %%f in ("%CARGO_TARGET_DIR%\release\bundle\nsis\*-setup.exe") do copy /y "%%f" "Sompio for Windows\" >nul
rem Only the newest setup file: no Key Maker, no other files
if exist "Sompio for Windows\For testers" rmdir /s /q "Sompio for Windows\For testers"
mkdir "Sompio for Windows\For testers"
for %%f in ("%CARGO_TARGET_DIR%\release\bundle\nsis\*-setup.exe") do copy /y "%%f" "Sompio for Windows\For testers\" >nul

echo.
echo Done. In the folder that opens:
echo   Sompio.exe              runs Sompio straight away
echo   Sompio_..._setup.exe    installs it with a Start menu shortcut (this is what testers get)
echo   Sompio Key Maker.exe    makes licence keys. Keep this one to yourself.
echo   For testers\            just the newest setup file. Send it with "How to test Sompio.txt".
echo.
echo Your signing key is in %APPDATA%\Sompio Key Maker. Back it up, never share it.
explorer "Sompio for Windows"
pause
exit /b 0

:fail
echo.
echo The build stopped. Send a screenshot of the red lines above.
pause
exit /b 1
