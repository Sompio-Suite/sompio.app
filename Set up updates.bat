@echo off
setlocal
title Set up Sompio updates
cd /d "%~dp0"

echo Sompio updates: one-time setup
echo ==============================
echo.
echo This makes the key that signs Sompio updates, and tells published releases where to look for them.
echo You only need it when you start publishing. Build Sompio.bat never needs it.
echo You only do this once.
echo.

where npm.cmd >nul 2>nul || (
  echo Node.js was not found. Install it from https://nodejs.org first.
  pause
  exit /b 1
)

set "KEYDIR=%APPDATA%\Sompio Key Maker"
set "KEY=%KEYDIR%\update.key"
if not exist "%KEYDIR%" mkdir "%KEYDIR%"

set /p OWNER=Your GitHub user name: 
set /p REPO=The GitHub repository customers download Sompio from (for example sompio-releases): 
if "%OWNER%"=="" goto :missing
if "%REPO%"=="" goto :missing

cd /d "%~dp0app"
if not exist node_modules call npm.cmd install --no-audit --no-fund >nul

if exist "%KEY%" (
  echo.
  echo An update key already exists, keeping it.
) else (
  echo.
  echo Making the update signing key...
  call npx.cmd tauri signer generate --ci -w "%KEY%" >nul || goto :fail
)

rem Put the public half of the key and the download address into the app's settings.
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$c = Get-Content -Raw 'src-tauri\tauri.release.conf.json' | ConvertFrom-Json;" ^
  "$c.plugins.updater.pubkey = (Get-Content -Raw '%KEY%.pub').Trim();" ^
  "$c.plugins.updater.endpoints = @('https://github.com/%OWNER%/%REPO%/releases/latest/download/latest.json');" ^
  "[IO.File]::WriteAllText((Resolve-Path 'src-tauri\tauri.release.conf.json'), ($c | ConvertTo-Json -Depth 20))" || goto :fail

echo.
echo Done. Two things left, both on github.com:
echo.
echo  1. In your Sompio code repository: Settings, Secrets and variables, Actions, New repository secret.
echo     Name: TAURI_SIGNING_PRIVATE_KEY
echo     Value: everything in the Notepad window that opens now. Then close Notepad.
echo.
echo  2. Commit and push the change to app\src-tauri\tauri.release.conf.json.
echo.
echo The key file is in %KEYDIR%. Back it up with your licence signing key. Never share it:
echo whoever has it can push updates to every Sompio user.
echo.
notepad "%KEY%"
pause
exit /b 0

:missing
echo Both answers are needed. Run this again.
pause
exit /b 1

:fail
echo.
echo Something went wrong. Send a screenshot of the lines above.
pause
exit /b 1
