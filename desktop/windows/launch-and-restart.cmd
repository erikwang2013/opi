@echo off
rem Launch the candidate window and restart Explorer so the new TSF DLL is reloaded.
start "" "%~dp0..\opi-candidates.exe"
taskkill /f /im explorer.exe >nul 2>&1
exit /b 0
