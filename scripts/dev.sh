#!/usr/bin/env bash
set -euo pipefail

echo "========================================"
echo "  SnapSift - Multimedia Organizer & Dedup Tool"
echo "  开发环境一键启动 (macOS / Linux)"
echo "========================================"
echo

# ---- 检查 Node.js ----
if ! command -v node &>/dev/null; then
    echo "[ERROR] 未检测到 Node.js，请先安装:"
    echo "        https://nodejs.org/"
    echo "        macOS:  brew install node"
    echo "        Linux:  sudo apt install nodejs npm  或  sudo dnf install nodejs npm"
    exit 1
fi
echo "[OK] Node.js $(node -v)"

# ---- 检查 npm ----
if ! command -v npm &>/dev/null; then
    echo "[ERROR] 未检测到 npm，请重新安装 Node.js"
    exit 1
fi
echo "[OK] npm $(npm -v)"

# ---- 检查 Rust ----
if ! command -v rustc &>/dev/null; then
    echo "[ERROR] 未检测到 Rust 工具链，请先安装:"
    echo "        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi
echo "[OK] $(rustc --version)"

# ---- 平台特定依赖检查 ----
OS="$(uname -s)"
case "$OS" in
    Darwin)
        echo "[OK] macOS 平台"
        if ! xcode-select -p &>/dev/null; then
            echo "[WARN] 未检测到 Xcode Command Line Tools"
            echo "       请运行: xcode-select --install"
        fi
        ;;
    Linux)
        echo "[OK] Linux 平台"
        MISSING=""
        for pkg in libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf; do
            if ! dpkg -s "$pkg" &>/dev/null 2>&1; then
                MISSING="$MISSING $pkg"
            fi
        done
        if [ -n "$MISSING" ]; then
            echo "[WARN] 可能缺少系统依赖:$MISSING"
            echo "       请运行: sudo apt install$MISSING"
            echo "       (如果不是 Debian/Ubuntu，请参考 Tauri 文档安装对应包)"
        fi
        ;;
esac

# ---- 进入项目根目录 ----
cd "$(dirname "$0")/.."

# ---- 安装 npm 依赖 ----
if [ ! -d "node_modules" ]; then
    echo
    echo "[INFO] 首次运行，安装 npm 依赖..."
    npm install
fi
echo "[OK] npm 依赖已就绪"

echo
echo "========================================"
echo "  所有依赖检查通过，正在启动..."
echo "  首次启动 Rust 编译可能需要 2-5 分钟"
echo "  启动后将自动弹出应用窗口"
echo "========================================"
echo

# ---- 启动 Tauri 开发模式 ----
npm run tauri dev
