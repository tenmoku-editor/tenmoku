@echo off
setlocal enabledelayedexpansion
REM Build script for the sample project
:: Another style of comment
set "ROOT=%~dp0"
set NAME=sample
set /a COUNT=0
set /a TOTAL=COUNT+5*2

if "%1"=="" goto :usage
if /i "%1"=="clean" (
    echo Cleaning %ROOT%build
    rmdir /s /q "%ROOT%build"
) else (
    echo Building %NAME%
)

if not exist "%ROOT%build" mkdir "%ROOT%build"
if %ERRORLEVEL% neq 0 exit /b 1

for /f "tokens=1,2 delims==" %%a in (config.ini) do (
    echo key %%a value %%b
    set /a COUNT+=1
)

for %%f in (*.txt) do echo %%f

call :helper arg1 arg2
call "%ROOT%tools\extra.cmd" --verbose

dir /b | findstr /i "txt" > "%TEMP%\list.txt" 2>&1
type nul > empty.log
echo Done: !COUNT! of %TOTAL% >> build.log

endlocal
exit /b 0

:helper
echo helper got %1 and %2
goto :eof

:usage
echo Usage: build.bat [clean]
exit /b 2
