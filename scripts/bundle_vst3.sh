#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
cargo build --release -p collapse
LIB="$ROOT/target/release/libcollapse.so"
OUT="$ROOT/dist/COLLAPSE.vst3/Contents/x86_64-linux"
mkdir -p "$OUT"
cp "$LIB" "$OUT/COLLAPSE.so"
# VST3 moduleinfo-less layout still loads in many Linux hosts / Yabridge
mkdir -p "$ROOT/dist/COLLAPSE.vst3/Contents"
cat > "$ROOT/dist/COLLAPSE.vst3/Contents/PkgInfo" <<'EOF'
BNDL????
EOF
echo "Bundled: $ROOT/dist/COLLAPSE.vst3"
echo "Copy into ~/.vst3/ or your DAW VST3 folder."
