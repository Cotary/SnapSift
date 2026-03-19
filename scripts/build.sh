#!/usr/bin/env bash
set -euo pipefail

echo "========================================"
echo "  SnapSift - Production Build (macOS / Linux)"
echo "========================================"
echo

cd "$(dirname "$0")/.."

if [ ! -d "node_modules" ]; then
    echo "[INFO] 安装 npm 依赖..."
    npm install
fi

echo "[INFO] 开始构建..."
echo "       这可能需要 5-10 分钟（Release 优化编译）"
echo

npm run tauri build

echo
echo "========================================"
echo "  构建完成！安装包位于:"
echo "  src-tauri/target/release/bundle/"
echo "========================================"
