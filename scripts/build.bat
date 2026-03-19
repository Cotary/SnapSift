@echo off
chcp 65001 >nul 2>&1
title Realphoto - 生产构建

echo ========================================
echo   Realphoto - 生产构建 (Windows)
echo ========================================
echo.

:: ---- 设置 MSVC 环境 ----
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
    call "%VCVARS%" >nul 2>&1
)

cd /d "%~dp0.."

if not exist "node_modules" (
    echo [INFO] 安装 npm 依赖...
    call npm install
)

echo [INFO] 开始构建...
echo        这可能需要 5-10 分钟（Release 优化编译）
echo.

call npm run tauri build

if %errorlevel% equ 0 (
    echo.
    echo ========================================
    echo   构建完成！安装包位于:
    echo   src-tauri\target\release\bundle\
    echo ========================================
) else (
    echo.
    echo [ERROR] 构建失败，请检查上方错误信息
)

pause
