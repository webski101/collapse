# Test live Atlas with the key saved at %USERPROFILE%\.config\collapse\atlas_key
# Run from the project root (folder that contains Cargo.toml).

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$keyPath = Join-Path $env:USERPROFILE ".config\collapse\atlas_key"
if (-not (Test-Path $keyPath)) {
    Write-Error "No key at $keyPath - run .\scripts\set_atlas_key.ps1 first"
}

$env:ATLAS_MODE = "live"
$env:ATLAS_BASE_URL = "https://api.mothquantum.com"
$env:ATLAS_ENGINE_ID = "retrocausal-echo-v1"

Write-Host "Project: $root"
Write-Host "Key file: $keyPath (contents not shown)"
Write-Host "Running atlas_ping..."
cargo run --release -p collapse --bin atlas_ping
if ($LASTEXITCODE -ne 0) {
    Write-Error "atlas_ping failed with exit code $LASTEXITCODE"
}
