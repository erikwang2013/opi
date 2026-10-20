@echo off
setlocal
rem 2026-10-20: never leave a white screen. The old script killed Explorer and did not
rem guarantee a restart; on systems with AutoRestartShell disabled the desktop stayed
rem white (and shell crash/restart loops burned CPU). Start the candidate only if it is
rem not already running, then restart Explorer with a bounded wait + explicit fallback.
tasklist /nh /fi "imagename eq opi.exe" 2>nul | find /i "opi.exe" >nul
if errorlevel 1 start "" "%~dp0..\opi.exe"
taskkill /f /im explorer.exe >nul 2>&1
set /a opi_tries=0
:opi_wait_shell
tasklist /nh /fi "imagename eq explorer.exe" 2>nul | find /i "explorer.exe" >nul
if not errorlevel 1 goto opi_shell_ok
set /a opi_tries+=1
rem 8 tasklist polls: fast machines are back in well under 2 s; slow ones get the
rem own-launch fallback. A longer wait only extends a white screen.
if %opi_tries% geq 8 goto opi_start_shell
ping -n 1 -w 200 127.0.0.1 >nul
goto opi_wait_shell
:opi_start_shell
rem No shell after the bounded poll (AutoRestartShell off, or Explorer crashed).
rem Start it ourselves with the current (user) token; never leave a white screen.
start "" "%SystemRoot%\explorer.exe"
:opi_shell_ok
exit /b 0
