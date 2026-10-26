@echo off
rem Double-click to build the portable SplitGate (runs tests first; stops if any fail).
rem Output: portable\SplitGate\ and portable\SplitGate-<version>-portable.zip
rem Usage: build.bat [--no-pause]
setlocal
pushd "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\build-portable.ps1"
set "RC=%ERRORLEVEL%"
popd
if not "%RC%"=="0" echo [ERROR] Build failed, see the messages above.
if /i not "%~1"=="--no-pause" pause
exit /b %RC%
