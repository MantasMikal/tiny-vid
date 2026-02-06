#!/usr/bin/env bash
# Build FFmpeg from source with LGPL + VideoToolbox only for macOS.
# Run on macOS. Output: src-tauri/binaries/standalone-lgpl-vt/ffmpeg-<target>, ffprobe-<target>
#
# Requires: xz, tar, make, nasm (for x86), or use --enable-lto and appropriate deps.
# For minimal deps, clone FFmpeg and run configure + make.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARIES_DIR="$ROOT/src-tauri/binaries/standalone-lgpl-vt"
BUILD_DIR="${FFMPEG_BUILD_DIR:-/tmp/ffmpeg-lgpl-build}"
# FFmpeg release branch (e.g. 8.0 → release/8.0). See https://git.ffmpeg.org/ffmpeg.git
FFMPEG_VERSION="${FFMPEG_VERSION:-8.0}"
TARGET_TRIPLE="${TARGET_TRIPLE:-$(rustc --print host-tuple 2>/dev/null || echo "aarch64-apple-darwin")}"
LGPL_DYLIBS=(
  "libavcodec"
  "libavdevice"
  "libavfilter"
  "libavformat"
  "libavutil"
  "libswresample"
  "libswscale"
)

echo "Building FFmpeg LGPL + VideoToolbox for $TARGET_TRIPLE (release/$FFMPEG_VERSION)"

mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

CURRENT_BRANCH="$(git -C ffmpeg rev-parse --abbrev-ref HEAD 2>/dev/null)" || true
if [ ! -d "ffmpeg" ] || [ "$CURRENT_BRANCH" != "release/$FFMPEG_VERSION" ]; then
  if [ -d "ffmpeg" ]; then
    echo "FFmpeg branch '$CURRENT_BRANCH' != 'release/$FFMPEG_VERSION'. Re-cloning..."
    rm -rf ffmpeg
  fi
  echo "Cloning FFmpeg release/$FFMPEG_VERSION..."
  git clone --depth 1 --branch "release/$FFMPEG_VERSION" https://git.ffmpeg.org/ffmpeg.git
fi

cd ffmpeg

./configure \
  --prefix="$BUILD_DIR/install" \
  --disable-gpl \
  --disable-nonfree \
  --disable-version3 \
  \
  --enable-shared \
  --disable-static \
  \
  --enable-videotoolbox \
  --enable-audiotoolbox \
  \
  --enable-encoder=h264_videotoolbox \
  --enable-encoder=hevc_videotoolbox \
  --enable-encoder=aac_at \
  \
  --enable-muxer=mp4 \
  --enable-muxer=matroska \
  --enable-demuxer=mov \
  --enable-demuxer=matroska \
  --enable-demuxer=srt \
  \
  --enable-protocol=file \
  \
  --disable-libx264 \
  --disable-libx265 \
  --disable-libsvtav1 \
  \
  --disable-doc \
  --disable-debug \
  --disable-ffplay \
  --disable-network

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

# Copy required shared libs and normalize install names for sidecar runtime.
for lib in "${LGPL_DYLIBS[@]}"; do
  SRC="$BUILD_DIR/install/lib/${lib}.dylib"
  if [[ ! -f "$SRC" ]]; then
    echo "error: missing required dylib: $SRC" >&2
    exit 1
  fi
  cp -L "$SRC" "$BINARIES_DIR/${lib}.dylib"
done

rewrite_install_names() {
  local file="$1"
  while IFS= read -r dep; do
    for lib in "${LGPL_DYLIBS[@]}"; do
      if [[ "$dep" == "$BUILD_DIR/install/lib/${lib}"*.dylib || "$dep" == "@rpath/${lib}"*.dylib ]]; then
        install_name_tool -change "$dep" "@loader_path/${lib}.dylib" "$file"
      fi
    done
  done < <(otool -L "$file" | awk 'NR>1 { print $1 }')
}

verify_install_names() {
  local file="$1"
  local unresolved
  unresolved="$(
    otool -L "$file" \
      | awk 'NR>1 { print $1 }' \
      | grep -E "^${BUILD_DIR}/install/lib/|^@rpath/(libav|libsw)" \
      || true
  )"
  if [[ -n "$unresolved" ]]; then
    echo "error: unresolved shared library references in $file" >&2
    echo "$unresolved" >&2
    exit 1
  fi
}

for lib in "${LGPL_DYLIBS[@]}"; do
  install_name_tool -id "@loader_path/${lib}.dylib" "$BINARIES_DIR/${lib}.dylib"
done

for file in "$BINARIES_DIR/ffmpeg-$SUFFIX" "$BINARIES_DIR/ffprobe-$SUFFIX"; do
  rewrite_install_names "$file"
  verify_install_names "$file"
done
for lib in "${LGPL_DYLIBS[@]}"; do
  file="$BINARIES_DIR/${lib}.dylib"
  rewrite_install_names "$file"
  verify_install_names "$file"
done

chmod +x "$BINARIES_DIR/ffmpeg-$SUFFIX" "$BINARIES_DIR/ffprobe-$SUFFIX"

echo "Done. Binaries at:"
echo "  $BINARIES_DIR/ffmpeg-$SUFFIX"
echo "  $BINARIES_DIR/ffprobe-$SUFFIX"
for lib in "${LGPL_DYLIBS[@]}"; do
  echo "  $BINARIES_DIR/${lib}.dylib"
done
