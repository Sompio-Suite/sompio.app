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

echo Building Sompio. The first time takes 5 to 10 minutes, later builds are quicker.
echo.

rem 1. The key maker, and the public half of your signing key for the app to trust.
echo Step 1 of 2: Sompio Key Maker
cd /d "%~dp0licence"
cargo build --release --bin sompio-keymaker || goto :fail
"target\release\sompio-keymaker.exe" init --public-out public.key >nul || goto :fail

rem 2. The app itself.
echo Step 2 of 2: Sompio
cd /d "%~dp0app"
call npm.cmd install --no-audit --no-fund || goto :fail
call npm.cmd run tauri build || goto :fail

cd /d "%~dp0"
if not exist "Sompio for Windows" mkdir "Sompio for Windows"
del /q "Sompio for Windows\*" >nul 2>nul
copy /y "app\src-tauri\target\release\sompio.exe" "Sompio for Windows\Sompio.exe" >nul
copy /y "licence\target\release\sompio-keymaker.exe" "Sompio for Windows\Sompio Key Maker.exe" >nul
for %%f in ("app\src-tauri\target\release\bundle\nsis\*-setup.exe") do copy /y "%%f" "Sompio for Windows\" >nul

echo.
echo Done. In the folder that opens:
echo   Sompio.exe              runs Sompio straight away
echo   Sompio_..._setup.exe    installs it with a Start menu shortcut (this is what testers get)
echo   Sompio Key Maker.exe    makes licence keys. Keep this one to yourself.
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
