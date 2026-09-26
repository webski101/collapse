#!/usr/bin/env bash
# Store your moth_ API key for COLLAPSE without exporting it every time.
# Usage: ./scripts/set_atlas_key.sh
#    or: ./scripts/set_atlas_key.sh 'moth_xxxxxxxx'
set -euo pipefail
DIR="${XDG_CONFIG_HOME:-$HOME/.config}/collapse"
mkdir -p "$DIR"
KEY_FILE="$DIR/atlas_key"

if [[ $# -ge 1 ]]; then
  KEY="$1"
else
  read -r -s -p "Paste moth_ API key (input hidden): " KEY
  echo
fi

if [[ -z "${KEY}" ]]; then
  echo "Empty key — aborted." >&2
  exit 1
fi
if [[ "$KEY" != moth_* ]]; then
  echo "Warning: key does not start with moth_ — continuing anyway."
fi

umask 077
printf '%s\n' "$KEY" >"$KEY_FILE"
chmod 600 "$KEY_FILE"
echo "Saved to $KEY_FILE"
echo "Test with: ATLAS_MODE=live cargo run --release -p collapse --bin atlas_ping"
