# Save Moth Atlas API key for COLLAPSE (Windows).
# Usage:
#   .\scripts\set_atlas_key.ps1
#   .\scripts\set_atlas_key.ps1 -Key 'moth_xxxxxxxx'

param(
    [string]$Key = ""
)

$ErrorActionPreference = "Stop"
$dir = Join-Path $env:USERPROFILE ".config\collapse"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$path = Join-Path $dir "atlas_key"

if ([string]::IsNullOrWhiteSpace($Key)) {
    $secure = Read-Host "Paste moth_ API key" -AsSecureString
    $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
    try {
        $Key = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr)
    } finally {
        [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr)
    }
}

$Key = $Key.Trim()
if ([string]::IsNullOrWhiteSpace($Key)) {
    Write-Error "Empty key — aborted."
}

if (-not $Key.StartsWith("moth_")) {
    Write-Warning "Key does not start with moth_ — saving anyway."
}

Set-Content -Path $path -Value $Key -NoNewline -Encoding ascii
Write-Host "Saved to $path"
Write-Host "Test with:"
Write-Host '  $env:ATLAS_MODE = "live"'
Write-Host "  cargo run --release -p collapse --bin atlas_ping"
