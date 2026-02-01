#!/usr/bin/env bash
# Dispatches to the platform-specific build script. All builds copy installers to releases/<platform>/.
# Usage: ./scripts/build.sh <full|bare|lgpl>
#   full  - macOS/Windows: bundle BtbN FFmpeg. Linux: not supported (use bare).
#   bare  - No bundled FFmpeg; use system FFmpeg.
#   lgpl  - macOS only: custom LGPL FFmpeg (App Store). Unsupported on Windows/Linux.

set -euo pipefail

VARIANT="${1:-}"
if [[ "$VARIANT" != "full" && "$VARIANT" != "bare" && "$VARIANT" != "lgpl" ]]; then
  echo "usage: $0 <full|bare|lgpl>" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

UNAME_S="$(uname -s 2>/dev/null || true)"
case "$UNAME_S" in
  Darwin)
    exec "$SCRIPT_DIR/build-macos.sh" "$VARIANT"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    if [[ "$VARIANT" == "lgpl" ]]; then
      echo "error: lgpl variant is macOS only; use full or bare on Windows" >&2
      exit 1
    fi
    exec "$SCRIPT_DIR/build-windows.sh" "$VARIANT"
    ;;
  Linux*)
    if [[ "$VARIANT" != "bare" ]]; then
      echo "error: only bare variant is supported on Linux; use ./scripts/build-linux.sh" >&2
      exit 1
    fi
    exec "$SCRIPT_DIR/build-linux.sh"
    ;;
  *)
    echo "error: unsupported platform: $UNAME_S" >&2
    exit 1
    ;;
esac
