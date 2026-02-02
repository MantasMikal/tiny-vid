#!/usr/bin/env bash
# Build Tiny Vid for Linux inside Docker. Produces .deb in releases/linux/.
# Run from repo root. Idempotent. Best run on Linux or in CI; mounting the repo
# on macOS will overwrite node_modules/target with Linux artifacts.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

IMAGE_NAME="${TINY_VID_LINUX_IMAGE:-tiny-vid-linux-builder}"
CONFIG_PATH="src-tauri/tauri.conf.json"

if ! command -v jq &>/dev/null; then
  echo "error: jq is required (install with your package manager)" >&2
  exit 1
fi

VERSION="$(jq -r '.version' "$CONFIG_PATH")"
if [[ "$VERSION" == "" || "$VERSION" == "null" ]]; then
  echo "error: could not read version from $CONFIG_PATH" >&2
  exit 1
fi

ARCH="$(uname -m)"
[[ "$ARCH" == "arm64" ]] && ARCH="aarch64"

echo "Building Linux .deb — version $VERSION ($ARCH)"

docker build -t "$IMAGE_NAME" -f docker/linux-builder.Dockerfile .

docker run --rm \
  -v "$REPO_ROOT:/app" \
  -w /app \
  "$IMAGE_NAME" \
  bash -c "yarn install && rm -rf src-tauri/target && yarn clean:bundle && yarn tauri build --no-bundle && yarn tauri bundle --bundles deb"

BUNDLE_DEB_DIR="src-tauri/target/release/bundle/deb"
if [[ ! -d "$BUNDLE_DEB_DIR" ]]; then
  echo "error: bundle output not found at $BUNDLE_DEB_DIR" >&2
  exit 1
fi

DEB_FILE="$(find "$BUNDLE_DEB_DIR" -maxdepth 1 -name '*.deb' -print -quit)"
if [[ -z "$DEB_FILE" ]]; then
  echo "error: no .deb file in $BUNDLE_DEB_DIR" >&2
  exit 1
fi

mkdir -p releases/linux
OUTPUT_NAME="releases/linux/Tiny-Vid-${VERSION}-linux-${ARCH}.deb"
cp -- "$DEB_FILE" "$OUTPUT_NAME"
echo "Output: $OUTPUT_NAME"
