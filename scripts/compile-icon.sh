#!/usr/bin/env bash
set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# Check for actool (requires macOS 26 + Xcode 26)
if ! command -v actool &> /dev/null; then
  echo "actool not found (requires Xcode 26). Skipping Assets.car generation."
  exit 0
fi

ICON_SRC="./src-tauri/icon-src/AppIcon.icon"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

# actool expects the folder to match --app-icon; copy to AppIcon.icon
cp -R "$ICON_SRC" "$TMP_DIR/AppIcon.icon"
ICON_PATH="$TMP_DIR/AppIcon.icon"
OUTPUT_PATH="$TMP_DIR/out"
PLIST_PATH="$OUTPUT_PATH/assetcatalog_generated_info.plist"
mkdir -p "$OUTPUT_PATH"

# Requires macOS 26 + Xcode 26
actool "$ICON_PATH" --compile "$OUTPUT_PATH" \
  --output-format human-readable-text --notices --warnings --errors \
  --output-partial-info-plist "$PLIST_PATH" \
  --app-icon AppIcon --include-all-app-icons \
  --enable-on-demand-resources NO --development-region en \
  --target-device mac --minimum-deployment-target 26.0 --platform macosx

mkdir -p "./src-tauri/icons"
cp "$OUTPUT_PATH/Assets.car" "./src-tauri/icons/Assets.car"
rm -rf "$TMP_DIR"
echo "Generated src-tauri/icons/Assets.car"
