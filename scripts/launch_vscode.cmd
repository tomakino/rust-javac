@echo off
setlocal
set "ELECTRON_RUN_AS_NODE="
set "ROOT=%~1"
if "%ROOT%"=="" set "ROOT=%~dp0.."
start "" "%LOCALAPPDATA%\Programs\Microsoft VS Code\Code.exe" --disable-extension redhat.java --new-window "%ROOT%"
