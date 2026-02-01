#!/usr/bin/env bash
# Build all three macOS variants (bare, full, lgpl) and compare DMG sizes.
# Run from repo root on macOS. Unset CI to avoid tauri --ci errors.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

REPORT="$REPO_ROOT/releases/macos/size-report.txt"
mkdir -p releases/macos
: > "$REPORT"

# Use same version source as build-macos.sh so expected paths match
VERSION="$(jq -r '.version' src-tauri/tauri.conf.json)"
if [[ "$VERSION" == "" || "$VERSION" == "null" ]]; then
  echo "error: could not read version from src-tauri/tauri.conf.json" >&2
  exit 1
fi

unset CI

for variant in bare full lgpl; do
  echo "Building macOS ($variant)..." >&2
  ./scripts/build-macos.sh "$variant" >&2
  DMG="releases/macos/Tiny-Vid-${VERSION}-macos-${variant}.dmg"
  if [[ -f "$DMG" ]]; then
    SIZE="$(stat -f%z "$DMG")"
    SIZE_H="$(ls -lh "$DMG" | awk '{print $5}')"
    echo "$variant $SIZE $SIZE_H $DMG" >> "$REPORT"
    echo "  $variant: $SIZE_H ($SIZE bytes)" >&2
  else
    echo "  $variant: (no DMG at $DMG)" >&2
  fi
done

echo ""
echo "=== macOS build size comparison ==="
printf "%-6s %12s %8s\n" "Variant" "Bytes" "Human"
echo "----------------------------------------"
while read -r variant size size_h _; do
  printf "%-6s %12s %8s\n" "$variant" "$size" "$size_h"
done < "$REPORT"
echo ""
echo "All DMGs in releases/macos:"
ls -la releases/macos/*.dmg 2>/dev/null || echo "  (none)"
echo ""
echo "Expected: bare < lgpl < full (bare=no ffmpeg, lgpl=lgpl ffmpeg, full=full ffmpeg)"
