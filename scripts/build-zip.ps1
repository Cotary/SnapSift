#Requires -Version 5.1
<#
.SYNOPSIS
  Build SnapSift and package as portable Windows ZIP.
.DESCRIPTION
  1. Optionally updates version in tauri.conf.json and package.json.
  2. Detects MSVC environment and initializes it.
  3. Builds the Tauri app (frontend + Rust + embed).
  4. Assembles files into dist\SnapSift\.
  5. Compresses into dist\SnapSift-vX.X.X-windows-x64.zip.
.EXAMPLE
  .\scripts\build-zip.ps1                        # Use version from tauri.conf.json
  .\scripts\build-zip.ps1 -Version 1.0.0         # Set version to 1.0.0 and build
  .\scripts\build-zip.ps1 -SkipBuild             # Package only, skip compile
  .\scripts\build-zip.ps1 -Version 1.0.0 -SkipBuild
#>
param(
    [string]$Version,
    [switch]$SkipBuild
)

Set-StrictMode -Off
$ErrorActionPreference = "Stop"

# ── helpers ──────────────────────────────────────────────────────────────
function Step($n, $msg) { Write-Host "`n[STEP $n] $msg" -ForegroundColor Cyan }
function Ok($msg)        { Write-Host "[OK] $msg"    -ForegroundColor Green }
function Info($msg)      { Write-Host "[INFO] $msg"  -ForegroundColor White }
function Warn($msg)      { Write-Host "[WARN] $msg"  -ForegroundColor Yellow }
function Fail($msg)      { Write-Host "[ERROR] $msg" -ForegroundColor Red; Read-Host "Press Enter to exit"; exit 1 }

# ── project root ─────────────────────────────────────────────────────────
$Root = Split-Path $PSScriptRoot -Parent
Set-Location $Root

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  SnapSift - Windows ZIP Packager"       -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

# ── resolve version ──────────────────────────────────────────────────────
$TauriConfPath = "src-tauri\tauri.conf.json"
$PkgJsonPath   = "package.json"

$conf = Get-Content $TauriConfPath -Raw | ConvertFrom-Json
$CurrentVersion = $conf.version
if (-not $CurrentVersion) { $CurrentVersion = "0.1.0" }

if (-not $Version) {
    Write-Host ""
    Write-Host "Current version: " -NoNewline -ForegroundColor White
    Write-Host "$CurrentVersion" -ForegroundColor Yellow
    $input_ver = Read-Host "Enter new version (press Enter to keep $CurrentVersion)"
    if ($input_ver) {
        $Version = $input_ver
    } else {
        $Version = $CurrentVersion
    }
}

# Validate version format (semver-like: digits.digits.digits)
if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    Fail "Invalid version format '$Version'. Expected: X.Y.Z (e.g. 1.0.0)"
}

# ── update version in config files if changed ────────────────────────────
if ($Version -ne $CurrentVersion) {
    Info "Updating version: $CurrentVersion -> $Version"

    # Update tauri.conf.json
    $tauriContent = Get-Content $TauriConfPath -Raw
    $tauriContent = $tauriContent -replace """version""\s*:\s*""[^""]+""", """version"": ""$Version"""
    $tauriContent | Set-Content $TauriConfPath -NoNewline
    Ok "Updated $TauriConfPath"

    # Update package.json
    $pkgContent = Get-Content $PkgJsonPath -Raw
    $pkgContent = $pkgContent -replace """version""\s*:\s*""[^""]+""", """version"": ""$Version"""
    $pkgContent | Set-Content $PkgJsonPath -NoNewline
    Ok "Updated $PkgJsonPath"
} else {
    Info "Version: $Version (unchanged)"
}

$AppDir  = "dist\SnapSift"
$ZipName = "SnapSift-v$Version-windows-x64.zip"
$ZipPath = "dist\$ZipName"

Info "Output  : $ZipPath"

# ── find MSVC vcvars64.bat ───────────────────────────────────────────────
$VcvarsPaths = @(
    "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
    "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat",
    "C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat",
    "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat"
)
$Vcvars = $VcvarsPaths | Where-Object { Test-Path $_ } | Select-Object -First 1

if ($Vcvars) {
    Info "Initializing MSVC..."
    $EnvLines = cmd /c "`"$Vcvars`" >nul 2>&1 && set" 2>$null
    foreach ($line in $EnvLines) {
        if ($line -match "^([^=]+)=(.*)$") {
            [System.Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], "Process")
        }
    }
    Ok "MSVC environment ready"
} else {
    Warn "MSVC not found. Rust compile may fail if kernel32.lib is missing."
    Warn "Install: winget install Microsoft.VisualStudio.2022.BuildTools"
}

# ── npm dependencies ─────────────────────────────────────────────────────
if (-not (Test-Path "node_modules")) {
    Info "Installing npm dependencies..."
    npm install
    if ($LASTEXITCODE -ne 0) { Fail "npm install failed" }
}

if (-not $SkipBuild) {
    # ── STEP 1+2: Tauri build (frontend + Rust + embed) ──────────────────
    Step "1+2" "Building Tauri app (frontend + Rust + embed)..."
    npx tauri build --no-bundle
    if ($LASTEXITCODE -ne 0) { Fail "Tauri build failed" }
    Ok "Tauri build done"
} else {
    Warn "Skipping build (-SkipBuild flag set)"
}

# ── verify exe ───────────────────────────────────────────────────────────
$ExePath = "src-tauri\target\release\app.exe"
if (-not (Test-Path $ExePath)) { Fail "app.exe not found at $ExePath. Please compile first." }

# ── STEP 3: assemble & zip ───────────────────────────────────────────────
Step 3 "Packaging files..."

if (Test-Path $AppDir) { Remove-Item $AppDir -Recurse -Force }
New-Item -ItemType Directory -Path "$AppDir\resources" -Force | Out-Null

Copy-Item $ExePath "$AppDir\SnapSift.exe"
Info "[+] SnapSift.exe"

$DmlSrc = "src-tauri\target\release\DirectML.dll"
if (Test-Path $DmlSrc) {
    Copy-Item $DmlSrc "$AppDir\DirectML.dll"
    Info "[+] DirectML.dll"
}

$OnnxSrc = "src-tauri\target\release\resources\mobilenet_v3_small.onnx"
if (-not (Test-Path $OnnxSrc)) {
    $OnnxSrc = "src-tauri\resources\mobilenet_v3_small.onnx"
}
if (Test-Path $OnnxSrc) {
    Copy-Item $OnnxSrc "$AppDir\resources\mobilenet_v3_small.onnx"
    Info "[+] resources\mobilenet_v3_small.onnx"
} else {
    Warn "ONNX model not found - AI dedup will be disabled"
}

@"
SnapSift v$Version

Usage: Double-click SnapSift.exe to launch.

Files:
  SnapSift.exe               Main application
  DirectML.dll               GPU acceleration for AI (optional)
  resources/                 AI model files

Requirements:
  Windows 10/11 x64
  WebView2 Runtime (pre-installed on most Win10/11 systems)
  Download WebView2: https://developer.microsoft.com/microsoft-edge/webview2/
"@ | Out-File "$AppDir\README.txt" -Encoding utf8
Info "[+] README.txt"

if (-not (Test-Path "dist")) { New-Item -ItemType Directory "dist" | Out-Null }
if (Test-Path $ZipPath) { Remove-Item $ZipPath -Force }
Compress-Archive -Path $AppDir -DestinationPath $ZipPath -Force

$SizeMB = [math]::Round((Get-Item $ZipPath).Length / 1MB, 1)

Write-Host "`n========================================" -ForegroundColor Green
Write-Host "  Done!" -ForegroundColor Green
Write-Host "  Version: v$Version" -ForegroundColor Green
Write-Host "  File: $ZipPath" -ForegroundColor Green
Write-Host "  Size: $SizeMB MB" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green

$open = Read-Host "`nOpen output folder? (y/n)"
if ($open -eq 'y') { Start-Process explorer "dist" }
