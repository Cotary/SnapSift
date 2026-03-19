@echo off
title Realphoto - Dev Mode

echo ========================================
echo   Realphoto - Dev Environment (Windows)
echo ========================================
echo.

:: --- Check Node.js ---
where node >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] Node.js not found. Install from:
    echo         https://nodejs.org/
    echo         or run: winget install OpenJS.NodeJS.LTS
    pause
    exit /b 1
)
for /f "tokens=*" %%v in ('node -v') do set NODE_VER=%%v
echo [OK] Node.js %NODE_VER%

:: --- Check npm ---
where npm >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] npm not found. Re-install Node.js.
    pause
    exit /b 1
)
echo [OK] npm ready

:: --- Check Rust ---
where rustc >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] Rust not found. Install from:
    echo         https://rustup.rs/
    echo         or run: winget install Rustlang.Rustup
    pause
    exit /b 1
)
for /f "tokens=*" %%v in ('rustc --version') do set RUST_VER=%%v
echo [OK] %RUST_VER%

:: --- Check MSVC ---
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
    echo [OK] MSVC Build Tools ready
    call "%VCVARS%" >nul 2>&1
) else (
    echo [WARN] MSVC Build Tools not found.
    echo        If Rust compile fails, install with:
    echo        winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
    echo.
)

:: --- Go to project root ---
cd /d "%~dp0.."

:: --- Install npm dependencies ---
if not exist "node_modules" (
    echo.
    echo [INFO] First run: installing npm dependencies...
    call npm install
    if %errorlevel% neq 0 (
        echo [ERROR] npm install failed
        pause
        exit /b 1
    )
)
echo [OK] npm dependencies ready

echo.
echo ========================================
echo   All checks passed. Starting dev mode...
echo   First Rust compile may take 2-5 minutes.
echo   App window will open automatically.
echo ========================================
echo.

call npm run tauri dev

pause
