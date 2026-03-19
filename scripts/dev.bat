@echo off
chcp 65001 >nul 2>&1
title Realphoto - 开发环境启动

echo ========================================
echo   Realphoto - 多媒体整理与去重工具
echo   开发环境一键启动 (Windows)
echo ========================================
echo.

:: ---- 检查 Node.js ----
where node >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] 未检测到 Node.js，请先安装:
    echo         https://nodejs.org/
    echo         或执行: winget install OpenJS.NodeJS.LTS
    pause
    exit /b 1
)
for /f "tokens=*" %%v in ('node -v') do set NODE_VER=%%v
echo [OK] Node.js %NODE_VER%

:: ---- 检查 npm ----
where npm >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] 未检测到 npm，请重新安装 Node.js
    pause
    exit /b 1
)
echo [OK] npm 已就绪

:: ---- 检查 Rust ----
where rustc >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] 未检测到 Rust 工具链，请先安装:
    echo         https://rustup.rs/
    echo         或执行: winget install Rustlang.Rustup
    pause
    exit /b 1
)
for /f "tokens=*" %%v in ('rustc --version') do set RUST_VER=%%v
echo [OK] %RUST_VER%

:: ---- 检查 MSVC 编译工具 ----
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

if defined VCVARS (
    echo [OK] MSVC Build Tools 已就绪
    call "%VCVARS%" >nul 2>&1
) else (
    echo [WARN] 未检测到 Visual Studio Build Tools
    echo        如果 Rust 编译失败，请安装:
    echo        winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
    echo.
)

:: ---- 进入项目根目录 ----
cd /d "%~dp0.."

:: ---- 安装 npm 依赖 ----
if not exist "node_modules" (
    echo.
    echo [INFO] 首次运行，安装 npm 依赖...
    call npm install
    if %errorlevel% neq 0 (
        echo [ERROR] npm install 失败
        pause
        exit /b 1
    )
)
echo [OK] npm 依赖已就绪

echo.
echo ========================================
echo   所有依赖检查通过，正在启动...
echo   首次启动 Rust 编译可能需要 2-5 分钟
echo   启动后将自动弹出应用窗口
echo ========================================
echo.

:: ---- 启动 Tauri 开发模式 ----
call npm run tauri dev

pause
