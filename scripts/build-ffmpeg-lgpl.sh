#!/usr/bin/env bash
# Build FFmpeg from source with LGPL + VideoToolbox only for macOS.
# Run on macOS. Output: src-tauri/binaries/ffmpeg-<target>, ffprobe-<target>
#
# Requires: xz, tar, make, nasm (for x86), or use --enable-lto and appropriate deps.
# For minimal deps, clone FFmpeg and run configure + make.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARIES_DIR="$ROOT/src-tauri/binaries"
BUILD_DIR="${FFMPEG_BUILD_DIR:-/tmp/ffmpeg-lgpl-build}"
# FFmpeg release branch (e.g. 7.1 → release/7.1). See https://git.ffmpeg.org/ffmpeg.git
FFMPEG_VERSION="${FFMPEG_VERSION:-7.1}"
TARGET_TRIPLE="${TARGET_TRIPLE:-$(rustc --print host-tuple 2>/dev/null || echo "aarch64-apple-darwin")}"

echo "Building FFmpeg LGPL + VideoToolbox for $TARGET_TRIPLE (release/$FFMPEG_VERSION)"

mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

if [ ! -d "ffmpeg" ]; then
  echo "Cloning FFmpeg release/$FFMPEG_VERSION..."
  git clone --depth 1 --branch "release/$FFMPEG_VERSION" https://git.ffmpeg.org/ffmpeg.git
fi

cd ffmpeg

./configure \
  --prefix="$BUILD_DIR/install" \
  --disable-gpl \
  --disable-nonfree \
  --enable-videotoolbox \
  --enable-encoder=h264_videotoolbox \
  --enable-encoder=hevc_videotoolbox \
  --enable-demuxer=mov \
  --enable-demuxer=matroska \
  --enable-muxer=mp4 \
  --enable-muxer=matroska \
  --disable-libx264 \
  --disable-libx265 \
  --disable-libsvtav1 \
  --disable-doc \
  --disable-ffplay

make -j"$(sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo 4)"
make install

mkdir -p "$BINARIES_DIR"

# Target triple suffix for sidecar naming
if [[ "$TARGET_TRIPLE" == *"windows"* ]]; then
  SUFFIX="${TARGET_TRIPLE}.exe"
else
  SUFFIX="$TARGET_TRIPLE"
fi

cp "$BUILD_DIR/install/bin/ffmpeg" "$BINARIES_DIR/ffmpeg-$SUFFIX"
cp "$BUILD_DIR/install/bin/ffprobe" "$BINARIES_DIR/ffprobe-$SUFFIX"

# Make executable (in case permissions were lost)
chmod +x "$BINARIES_DIR/ffmpeg-$SUFFIX" "$BINARIES_DIR/ffprobe-$SUFFIX"

echo "Done. Binaries at:"
echo "  $BINARIES_DIR/ffmpeg-$SUFFIX"
echo "  $BINARIES_DIR/ffprobe-$SUFFIX"
