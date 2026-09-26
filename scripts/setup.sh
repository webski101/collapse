#!/usr/bin/env bash
set -euo pipefail
echo "Rust $(rustc --version)"
echo "Fetching Cargo deps (nih-plug via git)…"
cargo fetch -p collapse
echo "Ready. Build with: cargo build --release -p collapse && ./scripts/bundle_vst3.sh"
