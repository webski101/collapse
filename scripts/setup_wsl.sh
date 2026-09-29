#!/usr/bin/env bash
# System deps for building COLLAPSE (nih-plug + egui) on Ubuntu/WSL.
set -euo pipefail
sudo apt-get update
sudo apt-get install -y \
  build-essential pkg-config \
  libasound2-dev \
  libgl1-mesa-dev \
  libx11-dev libx11-xcb-dev \
  libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxcursor-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libwayland-dev
echo "WSL deps installed. Next:"
echo "  cd ~/…/Collapse   # or /mnt/c/Users/USER/Collapse"
echo "  export ATLAS_MODE=live"
echo "  cargo run --release -p collapse --bin atlas_ping"
