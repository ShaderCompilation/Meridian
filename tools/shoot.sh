#!/usr/bin/env bash
# Usage: tools/shoot.sh <out-dir> "<script>"   (script syntax: see src/devtools.rs)
# Runs Meridian against a throwaway DB, replays the script, writes PNG screenshots.
set -euo pipefail
out="$1"; script="$2"
mkdir -p "$out"
bin="${MERIDIAN_BIN:-./target/debug/meridian}"
WGPU_BACKEND="${WGPU_BACKEND:-gl}" MERIDIAN_DB="${MERIDIAN_DB:-$out/test.db}" MERIDIAN_SHOTS="$out" \
  MERIDIAN_SCRIPT="$script" timeout 180 "$bin"
shopt -s nullglob
files=("$out"/*.rgba)
if ((${#files[@]})); then python3 "$(dirname "$0")/rgba2png.py" "${files[@]}"; fi
