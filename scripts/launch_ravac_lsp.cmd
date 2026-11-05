@echo off
setlocal
set "DIR=C:\Users\Lin\RustroverProjects\rust-javac\target\debug"
set "DST=%DIR%\ravac-lsp-live-%RANDOM%%RANDOM%.exe"
copy /y "%DIR%\ravac-lsp.exe" "%DST%" >nul
if errorlevel 1 exit /b 1
"%DST%" --class-path "C:\Users\Lin\RustroverProjects\rust-javac\target\tmp\perf\javac"
del "%DST%" >nul 2>&1
