@echo off
rem Builds Rusty Git Client into a standalone .exe and installers.
rem Double-click it, or run it from a terminal. Options are passed on, e.g.:
rem   scripts\build-release.cmd -Bundles none -Open
rem See build-release.ps1 for the options.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-release.ps1" %*
set EXITCODE=%ERRORLEVEL%
rem Keep the window open when it was started by double-clicking (no arguments), so the result can be
rem read. Set NOPAUSE=1 to never pause, e.g. when another program runs this script.
if "%~1"=="" if not defined NOPAUSE (
  echo %cmdcmdline% | find /i "%~nx0" >nul && pause
)
exit /b %EXITCODE%
