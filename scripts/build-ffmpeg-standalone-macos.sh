#!/usr/bin/env bash
# Build FFmpeg from source with GPL and common codecs (x264, x265, etc.) for macOS.
# Run on macOS. Output: native/binaries/standalone-gpl/ffmpeg-<target>, ffprobe-<target>
#
# Dependencies (Homebrew): x264, x265, libvpx, opus, svt-av1, dav1d, pkg-config
#   brew install x264 x265 libvpx opus svt-av1 dav1d pkg-config
# Optional: libvorbis for Vorbis audio (brew install libvorbis)
# For x86_64 native build you may need: nasm (brew install nasm)
#
# After running this, yarn build:standalone will use these binaries and skip downloading.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARIES_DIR="$ROOT/native/binaries/standalone-gpl"
BUILD_DIR="${FFMPEG_BUILD_DIR:-/tmp/ffmpeg-standalone-macos-build}"
# FFmpeg release branch (e.g. 8.0 → release/8.0). See https://git.ffmpeg.org/ffmpeg.git
FFMPEG_VERSION="${FFMPEG_VERSION:-8.0}"
TARGET_TRIPLE="${TARGET_TRIPLE:-$(rustc --print host-tuple 2>/dev/null || echo "aarch64-apple-darwin")}"

echo "Building FFmpeg GPL for standalone build — $TARGET_TRIPLE (release/$FFMPEG_VERSION)"

mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

if [ ! -d "ffmpeg" ]; then
  echo "Cloning FFmpeg release/$FFMPEG_VERSION..."
  git clone --depth 1 --branch "release/$FFMPEG_VERSION" https://git.ffmpeg.org/ffmpeg.git
fi

cd ffmpeg

./configure \
  --prefix="$BUILD_DIR/install" \
  --enable-gpl \
  --enable-version3 \
  --disable-nonfree \
  --enable-libx264 \
  --enable-libx265 \
  --enable-libvpx \
  --enable-libopus \
  --enable-libsvtav1 \
  --enable-libdav1d \
  --enable-videotoolbox \
  --enable-encoder=h264_videotoolbox \
  --enable-encoder=hevc_videotoolbox \
  --disable-doc \
  --disable-ffplay

make -j"$(sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo 4)"
make install

mkdir -p "$BINARIES_DIR"

if [[ "$TARGET_TRIPLE" == *"windows"* ]]; then
  SUFFIX="${TARGET_TRIPLE}.exe"
else
  SUFFIX="$TARGET_TRIPLE"
fi

cp "$BUILD_DIR/install/bin/ffmpeg" "$BINARIES_DIR/ffmpeg-$SUFFIX"
cp "$BUILD_DIR/install/bin/ffprobe" "$BINARIES_DIR/ffprobe-$SUFFIX"

chmod +x "$BINARIES_DIR/ffmpeg-$SUFFIX" "$BINARIES_DIR/ffprobe-$SUFFIX"

echo "Done. Binaries at:"
echo "  $BINARIES_DIR/ffmpeg-$SUFFIX"
echo "  $BINARIES_DIR/ffprobe-$SUFFIX"
echo "Run yarn build:standalone to use these binaries."
