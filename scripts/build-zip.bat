@echo off
title Realphoto - Build and Package ZIP

echo ========================================
echo   Realphoto - Windows ZIP Packager
echo ========================================
echo.

:: --- Project root (one level up from scripts\) ---
set "ROOT=%~dp0.."
cd /d "%ROOT%"

:: --- Read version from tauri.conf.json ---
set "VERSION=0.1.0"
for /f "tokens=2 delims=:, " %%v in ('findstr /i "\"version\"" src-tauri\tauri.conf.json') do (
    set "RAW=%%~v"
    set "RAW=!RAW: =!"
)
for /f "usebackq tokens=2 delims=:, " %%v in (`findstr /i "version" "src-tauri\tauri.conf.json"`) do (
    set "VERSION=%%~v"
    goto :ver_done
)
:ver_done

:: --- Output paths ---
set "OUT_DIR=%ROOT%\dist"
set "APP_DIR=%OUT_DIR%\Realphoto"
set "ZIP_NAME=Realphoto-v%VERSION%-windows-x64.zip"
set "ZIP_PATH=%OUT_DIR%\%ZIP_NAME%"

echo [INFO] Version : %VERSION%
echo [INFO] Output  : %ZIP_PATH%
echo.

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

:: --- Install npm dependencies ---
if not exist "node_modules" (
    echo [INFO] Installing npm dependencies...
    call npm install
    if %errorlevel% neq 0 (
        echo [ERROR] npm install failed
        pause
        exit /b 1
    )
)

:: --- Step 1: Build frontend ---
echo [STEP 1/3] Building frontend (Vite)...
call npm run build
if %errorlevel% neq 0 (
    echo [ERROR] Frontend build failed
    pause
    exit /b 1
)
echo [OK] Frontend build done.
echo.

:: --- Step 2: Compile Rust release binary ---
echo [STEP 2/3] Compiling Rust (cargo build --release)...
cargo build --release
if %errorlevel% neq 0 (
    echo [ERROR] Rust compile failed
    pause
    exit /b 1
)
echo [OK] Rust compile done.
echo.

:: --- Check output exe ---
set "EXE=src-tauri\target\release\app.exe"
if not exist "%EXE%" (
    echo [ERROR] %EXE% not found. Compile may have failed.
    pause
    exit /b 1
)

:: --- Step 3: Package into ZIP ---
echo [STEP 3/3] Packaging into ZIP...

if exist "%APP_DIR%" rd /s /q "%APP_DIR%"
mkdir "%APP_DIR%"
mkdir "%APP_DIR%\resources"

copy /y "src-tauri\target\release\app.exe" "%APP_DIR%\Realphoto.exe" >nul
echo [+] Realphoto.exe

if exist "src-tauri\target\release\DirectML.dll" (
    copy /y "src-tauri\target\release\DirectML.dll" "%APP_DIR%\DirectML.dll" >nul
    echo [+] DirectML.dll
)

if exist "src-tauri\target\release\resources\mobilenet_v3_small.onnx" (
    copy /y "src-tauri\target\release\resources\mobilenet_v3_small.onnx" "%APP_DIR%\resources\mobilenet_v3_small.onnx" >nul
    echo [+] resources\mobilenet_v3_small.onnx
) else if exist "src-tauri\resources\mobilenet_v3_small.onnx" (
    copy /y "src-tauri\resources\mobilenet_v3_small.onnx" "%APP_DIR%\resources\mobilenet_v3_small.onnx" >nul
    echo [+] resources\mobilenet_v3_small.onnx
) else (
    echo [WARN] mobilenet_v3_small.onnx not found. AI dedup will be unavailable.
)

(
echo Realphoto v%VERSION%
echo.
echo Usage: Double-click Realphoto.exe to launch.
echo.
echo Files:
echo   Realphoto.exe              - Main application
echo   DirectML.dll               - GPU acceleration for AI (optional)
echo   resources/                 - AI model files
echo.
echo Requirements:
echo   Windows 10/11 x64
echo   WebView2 Runtime (usually pre-installed on Win10/11)
echo   If WebView2 is missing: https://developer.microsoft.com/microsoft-edge/webview2/
) > "%APP_DIR%\README.txt"
echo [+] README.txt

if exist "%ZIP_PATH%" del /q "%ZIP_PATH%"
powershell -NoProfile -Command "Compress-Archive -Path '%APP_DIR%' -DestinationPath '%ZIP_PATH%' -Force"
if %errorlevel% neq 0 (
    echo [ERROR] ZIP packaging failed
    pause
    exit /b 1
)

for %%f in ("%ZIP_PATH%") do set "ZIP_BYTES=%%~zf"
set /a "ZIP_MB=%ZIP_BYTES% / 1048576"

echo.
echo ========================================
echo   Done!
echo   File : dist\%ZIP_NAME%
echo   Size : %ZIP_MB% MB
echo ========================================
echo.

set /p OPEN="Open output folder? (y/n): "
if /i "%OPEN%"=="y" explorer "%OUT_DIR%"

pause
