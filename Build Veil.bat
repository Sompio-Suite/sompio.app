@echo off
setlocal
title Building Veil
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

echo Building Veil. The first time takes 5 to 10 minutes, later builds are quicker.
echo.
cd /d "%~dp0app"
call npm.cmd install --no-audit --no-fund || goto :fail
call npm.cmd run tauri build || goto :fail

cd /d "%~dp0"
if not exist "Veil for Windows" mkdir "Veil for Windows"
del /q "Veil for Windows\*" >nul 2>nul
copy /y "app\src-tauri\target\release\veil.exe" "Veil for Windows\Veil.exe" >nul
for %%f in ("app\src-tauri\target\release\bundle\nsis\*-setup.exe") do copy /y "%%f" "Veil for Windows\" >nul

echo.
echo Done. In the folder that opens:
echo   Veil.exe            runs Veil straight away
echo   Veil_..._setup.exe  installs it with a Start menu shortcut (this is what you give to others)
explorer "Veil for Windows"
pause
exit /b 0

:fail
echo.
echo The build stopped. Send a screenshot of the red lines above.
pause
exit /b 1
