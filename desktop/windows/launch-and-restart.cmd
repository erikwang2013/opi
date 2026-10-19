@echo off
rem Launch the candidate window and restart Explorer so the new TSF DLL is reloaded.
rem Also install the bundled PUA font for the current user (best effort, never fatal).
if exist "%~dp0opi-emoji.ttf" (
    powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install-emoji-font.ps1" >nul 2>&1
)
start "" "%~dp0..\opi-candidates.exe"
taskkill /f /im explorer.exe >nul 2>&1
exit /b 0
