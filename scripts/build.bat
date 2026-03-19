@echo off
title SnapSift - Production Build

echo ========================================
echo   SnapSift - Production Build (Windows)
echo ========================================
echo.

cd /d "%~dp0.."

:: --- Find MSVC environment ---
set "VCVARS="
if exist "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" (
    set "VCVARS=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
)
if exist "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat" (
    set "VCVARS=C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
)
if exist "C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat" (
    set "VCVARS=C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat"
)
if exist "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat" (
    set "VCVARS=C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat"
)

if defined VCVARS (
    echo [INFO] Initializing MSVC: %VCVARS%
    call "%VCVARS%" >nul 2>&1
) else (
    echo [WARN] MSVC not found. Install Visual Studio Build Tools 2022 if compile fails.
)

if not exist "node_modules" (
    echo [INFO] Installing npm dependencies...
    call npm install
    if %errorlevel% neq 0 (
        echo [ERROR] npm install failed
        pause
        exit /b 1
    )
)

echo [INFO] Starting build (this may take 5-10 minutes)...
echo.

call npm run tauri build

if %errorlevel% equ 0 (
    echo.
    echo ========================================
    echo   Build successful!
    echo   Output: src-tauri\target\release\bundle\
    echo ========================================
) else (
    echo.
    echo [ERROR] Build failed. Check the error messages above.
)

pause
