@echo off
setlocal
title Set up Sompio updates
cd /d "%~dp0"

echo Sompio updates: one-time setup
echo ==============================
echo.
echo This makes, on this PC only:
echo   - the key that signs Sompio updates, so installed copies only accept updates from you
echo   - a code that lets the cloud build put new versions on sompio.org
echo   - a code built into Sompio, so sompio.org only hands updates to Sompio apps
echo Running it again keeps what it made the first time.
echo.

where npm.cmd >nul 2>nul || (
  echo Node.js was not found. Install it from https://nodejs.org first.
  pause
  exit /b 1
)

set "KEYDIR=%APPDATA%\Sompio Key Maker"
set "KEY=%KEYDIR%\update.key"
set "CODES=%KEYDIR%\publishing codes.txt"
if not exist "%KEYDIR%" mkdir "%KEYDIR%"

cd /d "%~dp0app"
if not exist node_modules call npm.cmd install --no-audit --no-fund >nul

if exist "%KEY%" (
  echo An update key already exists, keeping it.
) else (
  echo Making the update signing key...
  call npx.cmd tauri signer generate --ci -w "%KEY%" >nul || goto :fail
)

rem The public half of the key, sompio.org, and the two publishing codes (made once)
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\setup-updates.ps1" -KeyDir "%KEYDIR%" -Root "%~dp0." || goto :fail

rem The two public files the cloud build needs, so it uses your keys
cd /d "%~dp0"
set "OUT=%~dp0Public keys"
if not exist "%OUT%" mkdir "%OUT%"
copy /y "app\src-tauri\tauri.release.conf.json" "%OUT%\" >nul
if exist "licence\public.key" (copy /y "licence\public.key" "%OUT%\" >nul) else (
  echo.
  echo licence\public.key is missing: run Build Sompio.bat once first, then run this again.
  pause
  exit /b 1
)

echo.
echo Done. Three things left:
echo.
echo  1. Notepad opens with your publishing codes. Add the three GitHub secrets and the two
echo     config.php lines, exactly as the file says.
echo  2. Add the two files in the "Public keys" folder that opens to the code. They're public
echo     halves only: nothing in them can sign anything.
echo  3. Back up "%KEYDIR%". Never share it: whoever has it can make licence keys and updates.
echo.
explorer "%OUT%"
notepad "%CODES%"
pause
exit /b 0

:fail
echo.
echo Something went wrong. Send a screenshot of the lines above.
pause
exit /b 1
