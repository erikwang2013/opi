@echo off
setlocal
rem 2026-10-20 white-screen fix: this script now ONLY starts the candidate window.
rem It must never restart (or kill) Explorer. The previous version ended the shell
rem and relied on a "wait" that returned immediately for loopback, then started
rem Explorer itself. That could leave the desktop blank: the poll could still see
rem the dying process and exit, the fallback raced Winlogon, and a shell started
rem from the installer can carry the wrong token. TSF reloads the freshly
rem registered DLL on sign-out/reboot; the installer says so instead.
rem
rem The file name is historical (the MSI postprocess still looks it up by name).
tasklist /nh /fi "imagename eq opi.exe" 2>nul | find /i "opi.exe" >nul
if errorlevel 1 start "" "%~dp0..\opi.exe"
rem Start the statusbar as a second process; it hides until TSF reports im=opi
rem (see StatusBar.kt barVisible), so installing does not show a dead bar.
start "" "%~dp0..\opi.exe" --statusbar
exit /b 0
