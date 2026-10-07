@echo off
title AutoCRM - Backend + Frontend
cd /d "%~dp0"

start "AutoCRM Backend" cmd /k "cd /d backend && cargo run"
timeout /t 3 /nobreak >nul
start "AutoCRM Frontend" cmd /k "cd /d frontend && npm run dev"

echo.
echo   Backend:  http://localhost:8080
echo   Frontend: http://localhost:3000
echo.
echo   Close the terminal windows to stop the servers.
pause
