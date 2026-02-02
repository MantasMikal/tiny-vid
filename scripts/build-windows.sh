#!/usr/bin/env bash
# Build Tiny Vid for Windows locally. Produces .msi and .exe in releases/windows/.
# Usage: ./scripts/build-windows.sh [standalone]   (default: no arg = default)
# Run from repo root on Windows only (e.g. Git Bash, MSYS2). Signing not included.

set -euo pipefail

# Detect Windows (Git Bash, MSYS2, Cygwin)
UNAME_S="$(uname -s 2>/dev/null || true)"
case "$UNAME_S" in
  MINGW*|MSYS*|CYGWIN*) ;;
  *)
    echo "error: build-windows.sh must run on Windows (e.g. Git Bash)" >&2
    exit 1
    ;;
esac

VARIANT="${1:-}"
if [[ "$VARIANT" != "" && "$VARIANT" != "standalone" ]]; then
  echo "error: variant must be empty (default) or standalone (got: $VARIANT)" >&2
  exit 1
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

CONFIG_PATH="src-tauri/tauri.conf.json"

# Use Node to read version (jq often not installed on Windows)
VERSION="$(node -e "console.log(require('./$CONFIG_PATH').version)")"
if [[ -z "$VERSION" ]]; then
  echo "error: could not read version from $CONFIG_PATH" >&2
  exit 1
fi

ARCH="$(uname -m 2>/dev/null || echo "x86_64")"
[[ "$ARCH" == "arm64" ]] && ARCH="aarch64"

if [[ -z "$VARIANT" ]]; then
  SUFFIX="windows-${ARCH}"
else
  SUFFIX="windows-${VARIANT}-${ARCH}"
fi

if [[ -z "$VARIANT" ]]; then
  echo "Building Windows installers (default) — version $VERSION ($ARCH)"
else
  echo "Building Windows installers ($VARIANT) — version $VERSION ($ARCH)"
fi

# Avoid tauri receiving CI=1 (invalid --ci 1); use unset so local/CI both work
unset CI

yarn clean:bundle
if [[ "$VARIANT" == "standalone" ]]; then
  yarn prepare-ffmpeg
  yarn tauri build --config src-tauri/overrides/windows-standalone.json
else
  yarn tauri build
fi

BUNDLE_DIR="src-tauri/target/release/bundle"
MSI_DIR="$BUNDLE_DIR/msi"
NSIS_DIR="$BUNDLE_DIR/nsis"

mkdir -p releases/windows

if [[ -d "$MSI_DIR" ]]; then
  MSI_FILE="$(find "$MSI_DIR" -maxdepth 1 -name '*.msi' -print -quit)"
  if [[ -n "$MSI_FILE" ]]; then
    cp -- "$MSI_FILE" "releases/windows/Tiny-Vid-${VERSION}-${SUFFIX}.msi"
    echo "Output: releases/windows/Tiny-Vid-${VERSION}-${SUFFIX}.msi"
  fi
fi

if [[ -d "$NSIS_DIR" ]]; then
  EXE_FILE="$(find "$NSIS_DIR" -maxdepth 1 -name '*.exe' -print -quit)"
  if [[ -n "$EXE_FILE" ]]; then
    cp -- "$EXE_FILE" "releases/windows/Tiny-Vid-${VERSION}-${SUFFIX}.exe"
    echo "Output: releases/windows/Tiny-Vid-${VERSION}-${SUFFIX}.exe"
  fi
fi
