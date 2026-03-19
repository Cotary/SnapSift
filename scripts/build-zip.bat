@echo off
:: Launcher for build-zip.ps1
:: Can be double-clicked or run from CMD / PowerShell
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-zip.ps1" %*
