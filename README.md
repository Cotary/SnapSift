# SnapSift — Multimedia Organizer & Dedup Tool

Cross-platform desktop app for organizing photos/videos by date and intelligently finding & cleaning similar images using perceptual hashing (pHash) + AI vector similarity.

**Tech Stack**: Tauri 2.0 · Rust · React · TypeScript · Vite · Tailwind CSS · shadcn/ui · SQLite

---

## Features

| Feature | Description |
|---------|-------------|
| Project Management | Create multiple projects, each with multiple source folders and one target folder |
| File Scanning | Recursively scan JPG/PNG/HEIC/WebP/MP4/MOV, extract EXIF date, pHash, MD5 |
| Date Organization | Templates like `YYYY/MM/DD`, `YYYY_MM`, etc. Copy or Move mode with auto-rename on conflict |
| Similar Image Dedup | pHash + AI (MobileNet-v3) Complete-Linkage clustering, user-configurable thresholds |
| Thumbnail Preview | Rust backend generates compressed thumbnails with LRU cache |
| Hover Preview | Full-resolution original image preview on hover |

---

## Requirements

| Dependency | Version | Install |
|------------|---------|---------|
| Node.js | >= 18 | [nodejs.org](https://nodejs.org/) |
| Rust | >= 1.77 | [rustup.rs](https://rustup.rs/) |
| **Windows** | MSVC Build Tools | `winget install Microsoft.VisualStudio.2022.BuildTools` |
| **macOS** | Xcode CLT | `xcode-select --install` |
| **Linux** | System libs | See below |

### Linux (Debian/Ubuntu)

```bash
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

---

## Quick Start

### Windows

```powershell
.\scripts\dev.bat
```

### macOS / Linux

```bash
chmod +x scripts/dev.sh
./scripts/dev.sh
```

### Manual (all platforms)

```bash
npm install        # first time only
npm run tauri dev  # starts Rust backend + Vite frontend
```

> First Rust compile takes ~2-5 minutes. Incremental builds are fast.

---

## Production Build

### Windows — Portable ZIP

```powershell
.\scripts\build-zip.ps1
# Output: dist\SnapSift-v0.1.0-windows-x64.zip
```

### Windows — MSI Installer

```powershell
.\scripts\build.bat
# Output: src-tauri\target\release\bundle\msi\
```

### macOS / Linux

```bash
chmod +x scripts/build.sh
./scripts/build.sh
```

Build output in `src-tauri/target/release/bundle/`:
- **Windows**: `.msi` installer
- **macOS**: `.dmg` / `.app`
- **Linux**: `.deb` / `.AppImage`

---

## Usage Guide

### 1. Create a Project

Launch the app and click "New Project", enter a project name.

### 2. Configure Folders

- **Source folders**: Click "Add Folder" to select directories containing photos/videos (supports multiple)
- **Target folder**: Click "Select Folder" to set the output directory

### 3. Scan Files

Click "Start Scan" to:
- Recursively traverse all source folders
- Extract EXIF capture date (falls back to file modified time)
- Compute pHash and MD5 for each image
- Show real-time scan progress

### 4. Organize by Date

After scanning, in the "Organize" panel:
1. Select a date template (e.g. `YYYY/MM/DD`)
2. Choose operation mode (Copy / Move)
3. Click "Start Organize"

### 5. Similar Image Dedup

Click "Similar Images" to enter the dedup page:
1. Adjust pHash threshold (2-16) and AI similarity (80%-99%) sliders
2. Click "Re-analyze" to run detection
3. **Left-click** an image = keep only this one, mark others for deletion
4. **Right-click** an image = toggle keep/delete status manually
5. Use "Select All" / "Deselect All" / "Skip Below" for batch operations
6. Click "Execute Delete" to review and confirm physical deletion

---

## Project Structure

```
SnapSift/
├── scripts/                   # Build & dev scripts
│   ├── dev.bat / dev.sh       # Dev environment launcher
│   ├── build.bat / build.sh   # Production build
│   ├── build-zip.ps1          # Windows portable ZIP packager
│   └── export_model.py        # ONNX model export script
├── src/                       # React frontend
│   ├── pages/
│   │   ├── ProjectList.tsx        # Project list
│   │   ├── ProjectDetail.tsx      # Project detail (scan + organize)
│   │   └── DuplicateReview.tsx    # Similar image review
│   ├── components/
│   │   ├── DuplicateGroupCard.tsx # Duplicate group card
│   │   ├── OrganizePanel.tsx      # Date organize panel
│   │   ├── ScanProgress.tsx       # Scan progress bar
│   │   └── ThumbnailImage.tsx     # Thumbnail loader
│   └── lib/
│       ├── types.ts               # TypeScript types
│       └── commands.ts            # Tauri command wrappers
├── src-tauri/                 # Rust backend
│   └── src/
│       ├── lib.rs                 # Tauri entry point
│       ├── db.rs                  # SQLite database
│       ├── models.rs              # Data models
│       ├── commands.rs            # Tauri commands
│       ├── scanner.rs             # File scan engine
│       ├── organizer.rs           # Date organizer
│       ├── dedup.rs               # pHash + AI dedup
│       ├── embedder.rs            # AI feature extraction
│       └── thumbnail.rs           # Thumbnail generator
├── DEDUP_LOGIC.md             # Dedup algorithm documentation
└── package.json
```

---

## AI Model

The app uses MobileNet-v3-Small for AI-based image similarity detection. The ONNX model file (`mobilenet_v3_small.onnx`) should be placed in `src-tauri/resources/`.

To export the model yourself:

```bash
pip install torch torchvision onnx
python scripts/export_model.py
```

---

## License

MIT
