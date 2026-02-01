#!/usr/bin/env bash
# Build Tiny Vid for macOS locally. Produces .dmg in releases/macos/.
# Usage: ./scripts/build-macos.sh [full|bare|lgpl]   (default: bare)
# Run from repo root on macOS only. No Docker. Signing/notarization not included.

set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: build-macos.sh must run on macOS" >&2
  exit 1
fi

VARIANT="${1:-bare}"
if [[ "$VARIANT" != "full" && "$VARIANT" != "bare" && "$VARIANT" != "lgpl" ]]; then
  echo "error: variant must be full, bare, or lgpl (got: $VARIANT)" >&2
  exit 1
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
SRC_TAURI="src-tauri"

if ! command -v jq &>/dev/null; then
  echo "error: jq is required (install with brew install jq)" >&2
  exit 1
fi

VERSION="$(jq -r '.version' "$SRC_TAURI/tauri.conf.json")"
if [[ "$VERSION" == "" || "$VERSION" == "null" ]]; then
  echo "error: could not read version from $SRC_TAURI/tauri.conf.json" >&2
  exit 1
fi

echo "Building macOS ($VARIANT) — version $VERSION"

# Avoid tauri receiving CI=1 (invalid --ci 1); use unset so local/CI both work
unset CI

yarn clean:bundle

BUILD_EXIT=0
case "$VARIANT" in
  full)
    yarn prepare-ffmpeg
    yarn tauri build --config "$SRC_TAURI/overrides/macos-full.json" || BUILD_EXIT=$?
    ;;
  bare)
    yarn tauri build || BUILD_EXIT=$?
    ;;
  lgpl)
    TINY_VID_LGPL_MACOS=1 yarn prepare-ffmpeg
    yarn tauri build --config "$SRC_TAURI/overrides/macos-lgpl.json" --features lgpl-macos || BUILD_EXIT=$?
    ;;
esac

BUNDLE_DIR="$SRC_TAURI/target/release/bundle"
DMG_DIR="$BUNDLE_DIR/dmg"
SUFFIX="macos-${VARIANT}"

# Always create releases/macos and copy whatever was built (so releases/ exists even if DMG failed)
mkdir -p releases/macos

if [[ -d "$DMG_DIR" ]]; then
  DMG_FILE="$(find "$DMG_DIR" -maxdepth 1 -name '*.dmg' -print -quit)"
  if [[ -n "$DMG_FILE" ]]; then
    cp -- "$DMG_FILE" "releases/macos/Tiny-Vid-${VERSION}-${SUFFIX}.dmg"
    echo "Output: releases/macos/Tiny-Vid-${VERSION}-${SUFFIX}.dmg"
  fi
fi

exit "$BUILD_EXIT"
