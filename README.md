# Realphoto — 多媒体整理与去重工具

跨平台桌面应用，帮助你按日期自动归类照片/视频，并通过感知哈希（pHash）智能查找和清理相似图片。

**技术栈**: Tauri 2.0 · Rust · React · TypeScript · Vite · Tailwind CSS · shadcn/ui · SQLite

---

## 功能概览

| 功能 | 说明 |
|------|------|
| 项目管理 | 创建多个整理项目，每个项目管理多个源文件夹和一个目标文件夹 |
| 文件扫描 | 递归扫描 JPG/PNG/HEIC/WebP/MP4/MOV 等格式，提取 EXIF 拍摄时间、pHash、MD5 |
| 按日期整理 | 支持 `YYYY/MM/DD`、`YYYY_MM` 等模板，Copy 或 Move 模式，自动冲突重命名 |
| 相似图片去重 | pHash 汉明距离聚类，左键一键保留、右键手动切换，二次确认后物理删除 |
| 缩略图预览 | Rust 后端实时生成压缩缩略图，带 LRU 缓存，避免前端加载大图 |

---

## 环境要求

| 依赖 | 版本 | 安装方式 |
|------|------|---------|
| Node.js | ≥ 18 | [nodejs.org](https://nodejs.org/) |
| Rust | ≥ 1.77 | [rustup.rs](https://rustup.rs/) |
| **Windows 额外** | MSVC Build Tools | `winget install Microsoft.VisualStudio.2022.BuildTools` |
| **macOS 额外** | Xcode CLT | `xcode-select --install` |
| **Linux 额外** | 系统库 | 见下方说明 |

### Linux 系统依赖（Debian/Ubuntu）

```bash
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

---

## 一键启动

项目提供了 `scripts/` 目录下的启动脚本，自动检查环境依赖并启动开发服务器。

### Windows

双击运行，或在终端中执行：

```powershell
.\scripts\dev.bat
```

### macOS / Linux

```bash
chmod +x scripts/dev.sh
./scripts/dev.sh
```

### 手动启动（所有平台通用）

```bash
# 1. 安装前端依赖（仅首次）
npm install

# 2. 启动开发模式（同时编译 Rust 后端 + 启动 Vite 前端）
npm run tauri dev
```

> 首次启动 Rust 编译约需 2-5 分钟，后续增量编译很快。

---

## 生产构建

### Windows

```powershell
.\scripts\build.bat
```

### macOS / Linux

```bash
chmod +x scripts/build.sh
./scripts/build.sh
```

### 手动构建

```bash
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`：
- **Windows**: `.msi` 和 `.exe` 安装包
- **macOS**: `.dmg` 和 `.app`
- **Linux**: `.deb` 和 `.AppImage`

---

## 使用指南

### 1. 创建项目

启动应用后点击「新建项目」，输入项目名称。

### 2. 配置文件夹

- **源文件夹**: 点击「添加文件夹」选择包含照片/视频的目录（支持多个）
- **目标文件夹**: 点击「选择文件夹」设置整理后的输出目录

### 3. 扫描文件

点击「开始扫描」，程序会：
- 递归遍历所有源文件夹
- 提取 EXIF 拍摄日期（失败则取文件创建/修改时间）
- 为每张图片计算 pHash 感知哈希和 MD5 校验值
- 实时显示扫描进度

### 4. 按日期整理

扫描完成后，在「按日期整理」面板中：
1. 选择日期模板（如 `YYYY/MM/DD`）
2. 选择操作模式（复制 / 移动）
3. 点击「开始整理」

文件将按拍摄日期自动归类到目标文件夹的子目录中。

### 5. 相似图片去重

点击「相似图片筛选」卡片进入去重页面：
1. 点击「重新分析」执行 pHash 聚类（汉明距离 ≤ 8 归为一组）
2. **左键点击**某张图片 = 仅保留该图，组内其他标记为待删
3. **右键点击**某张图片 = 手动切换该图的保留/删除状态
4. 底部显示待删文件数量和预计释放空间
5. 点击「执行删除」后弹出确认列表，确认后物理删除

---

## 项目结构

```
e:\Realphoto/
├── scripts/                # 一键启动/构建脚本
│   ├── dev.bat / dev.sh    # 开发环境启动
│   └── build.bat / build.sh # 生产构建
├── src/                    # React 前端
│   ├── pages/              # 页面组件
│   │   ├── ProjectList.tsx     # 项目列表
│   │   ├── ProjectDetail.tsx   # 项目详情（扫描+整理入口）
│   │   └── DuplicateReview.tsx # 相似图片筛选
│   ├── components/         # 可复用组件
│   │   ├── ScanProgress.tsx    # 扫描进度条
│   │   ├── OrganizePanel.tsx   # 日期整理面板
│   │   ├── DuplicateGroupCard.tsx # 相似组展示
│   │   └── ThumbnailImage.tsx  # 缩略图加载
│   └── lib/                # 工具层
│       ├── types.ts            # TypeScript 类型
│       └── commands.ts         # Tauri 命令封装
├── src-tauri/              # Rust 后端
│   └── src/
│       ├── lib.rs              # Tauri 入口
│       ├── db.rs               # SQLite 数据库
│       ├── models.rs           # 数据模型
│       ├── commands.rs         # Tauri 命令
│       ├── scanner.rs          # 文件扫描引擎
│       ├── organizer.rs        # 时间轴整理
│       ├── dedup.rs            # pHash 聚类去重
│       └── thumbnail.rs        # 缩略图生成
└── package.json
```
