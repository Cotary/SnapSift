@echo off
chcp 65001 >nul 2>&1
title Realphoto - 构建并打包为 ZIP

echo ========================================
echo   Realphoto - Windows ZIP 打包脚本
echo ========================================
echo.

:: ---- 项目根目录（脚本所在目录的上一级）----
set "ROOT=%~dp0.."
cd /d "%ROOT%"

:: ---- 读取版本号（从 tauri.conf.json）----
for /f "tokens=2 delims=:, " %%v in ('findstr /i "\"version\"" src-tauri\tauri.conf.json') do (
    set "RAW_VER=%%v"
)
set "VERSION=%RAW_VER:"=%"
if "%VERSION%"=="" set "VERSION=0.1.0"
echo [INFO] 版本号: %VERSION%

:: ---- 输出目录 ----
set "OUT_DIR=%ROOT%\dist"
set "APP_DIR=%OUT_DIR%\Realphoto"
set "ZIP_NAME=Realphoto-v%VERSION%-windows-x64.zip"
set "ZIP_PATH=%OUT_DIR%\%ZIP_NAME%"

:: ---- 寻找 MSVC 环境 ----
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
    echo [INFO] 初始化 MSVC 环境: %VCVARS%
    call "%VCVARS%" >nul 2>&1
) else (
    echo [WARN] 未找到 MSVC 环境，若编译失败请安装 Visual Studio Build Tools 2022
)

:: ---- 安装 npm 依赖 ----
if not exist "node_modules" (
    echo [INFO] 安装 npm 依赖...
    call npm install
    if %errorlevel% neq 0 (
        echo [ERROR] npm install 失败
        pause
        exit /b 1
    )
)

:: ---- 编译 Release ----
echo.
echo [INFO] 开始 Release 编译（约 5~10 分钟）...
echo        前端 Vite 构建 + Rust cargo build --release
echo.

:: 只编译 exe，不触发 Tauri bundle（跳过 NSIS/MSI 下载）
call npm run build
if %errorlevel% neq 0 (
    echo [ERROR] 前端构建失败
    pause
    exit /b 1
)

set "CARGO_BUILD_CMD=cargo build --release"
if defined VCVARS (
    call cargo build --release
) else (
    cargo build --release
)
if %errorlevel% neq 0 (
    echo [ERROR] Rust 编译失败
    pause
    exit /b 1
)

:: ---- 检查产物 ----
set "EXE=%ROOT%\src-tauri\target\release\app.exe"
if not exist "%EXE%" (
    echo [ERROR] 未找到 %EXE%，编译可能未成功
    pause
    exit /b 1
)

:: ---- 清理并创建输出目录 ----
echo.
echo [INFO] 整理文件到 %APP_DIR% ...
if exist "%APP_DIR%" rd /s /q "%APP_DIR%"
mkdir "%APP_DIR%"
mkdir "%APP_DIR%\resources"

:: ---- 复制文件 ----
copy /y "%ROOT%\src-tauri\target\release\app.exe" "%APP_DIR%\Realphoto.exe" >nul
echo        [+] Realphoto.exe

:: DirectML.dll（ORT GPU 加速，若不存在则跳过）
if exist "%ROOT%\src-tauri\target\release\DirectML.dll" (
    copy /y "%ROOT%\src-tauri\target\release\DirectML.dll" "%APP_DIR%\DirectML.dll" >nul
    echo        [+] DirectML.dll
)

:: ONNX 模型
if exist "%ROOT%\src-tauri\target\release\resources\mobilenet_v3_small.onnx" (
    copy /y "%ROOT%\src-tauri\target\release\resources\mobilenet_v3_small.onnx" "%APP_DIR%\resources\mobilenet_v3_small.onnx" >nul
    echo        [+] resources\mobilenet_v3_small.onnx
) else if exist "%ROOT%\src-tauri\resources\mobilenet_v3_small.onnx" (
    copy /y "%ROOT%\src-tauri\resources\mobilenet_v3_small.onnx" "%APP_DIR%\resources\mobilenet_v3_small.onnx" >nul
    echo        [+] resources\mobilenet_v3_small.onnx (from src-tauri/resources)
) else (
    echo [WARN] 未找到 mobilenet_v3_small.onnx，AI 相似度检测将不可用
    echo        请将模型文件放入 resources\ 目录
)

:: 写入使用说明
(
echo Realphoto v%VERSION% - 多媒体整理与去重工具
echo.
echo 使用方法：
echo   双击 Realphoto.exe 启动程序
echo.
echo 目录结构：
echo   Realphoto.exe            -- 主程序
echo   DirectML.dll             -- AI GPU 加速（可选）
echo   resources/               -- AI 模型文件
echo.
echo 系统要求：
echo   Windows 10/11 x64
echo   WebView2 运行时（Win10/11 系统通常已内置）
echo   如未安装 WebView2 请访问: https://developer.microsoft.com/microsoft-edge/webview2/
) > "%APP_DIR%\使用说明.txt"
echo        [+] 使用说明.txt

:: ---- 打包 ZIP ----
echo.
echo [INFO] 打包为 ZIP...
if exist "%ZIP_PATH%" del /q "%ZIP_PATH%"

:: 使用 PowerShell 压缩（Windows 内置，无需额外工具）
powershell -NoProfile -Command ^
    "Compress-Archive -Path '%APP_DIR%' -DestinationPath '%ZIP_PATH%' -Force"

if %errorlevel% neq 0 (
    echo [ERROR] ZIP 打包失败
    pause
    exit /b 1
)

:: ---- 计算文件大小 ----
for %%f in ("%ZIP_PATH%") do set "ZIP_SIZE=%%~zf"
set /a "ZIP_MB=%ZIP_SIZE% / 1048576"

echo.
echo ========================================
echo   打包完成！
echo   文件: dist\%ZIP_NAME%
echo   大小: %ZIP_MB% MB
echo ========================================
echo.

:: 询问是否打开输出目录
set /p OPEN="是否打开输出目录？(y/n): "
if /i "%OPEN%"=="y" explorer "%OUT_DIR%"

pause
