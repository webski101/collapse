# Build COLLAPSE VST3 for Windows and copy into dist\
# Requires: Rust + MSVC build tools (Visual Studio Build Tools with C++).
# Run from anywhere; script cds to repo root.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

Write-Host "Building release plugin..."
cargo build --release -p collapse

$dll = Join-Path $root "target\release\collapse.dll"
if (-not (Test-Path $dll)) {
    Write-Error "Missing $dll — build failed?"
}

$bundle = Join-Path $root "dist\COLLAPSE.vst3\Contents\x86_64-win"
New-Item -ItemType Directory -Force -Path $bundle | Out-Null
Copy-Item $dll (Join-Path $bundle "COLLAPSE.vst3") -Force

Write-Host "Bundled: $root\dist\COLLAPSE.vst3"
Write-Host "Copy that folder into:"
Write-Host "  C:\Program Files\Common Files\VST3\"
Write-Host "Then rescan plugins in your DAW."
