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

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARIES_DIR="$ROOT/native/binaries/standalone-gpl"
BUILD_DIR="${FFMPEG_BUILD_DIR:-/tmp/ffmpeg-standalone-macos-build}"
# FFmpeg release branch (e.g. 8.0 → release/8.0). See https://git.ffmpeg.org/ffmpeg.git
FFMPEG_VERSION="${FFMPEG_VERSION:-8.0}"
TARGET_TRIPLE="${TARGET_TRIPLE:-$(rustc --print host-tuple 2>/dev/null || echo "aarch64-apple-darwin")}"
CODESIGN_IDENTITY="${TINY_VID_CODESIGN_IDENTITY:-${CODESIGN_IDENTITY:-}}"

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

# Bundle external codec dylibs next to ffmpeg so the app runs without Homebrew.
if ! command -v python3 >/dev/null 2>&1; then
  echo "error: python3 is required to collect dylib dependencies" >&2
  exit 1
fi

rm -f "$BINARIES_DIR"/*.dylib

export FFMPEG_BIN="$BINARIES_DIR/ffmpeg-$SUFFIX"
export FFPROBE_BIN="$BINARIES_DIR/ffprobe-$SUFFIX"

DEPS="$(
  python3 - <<'PY'
import os
import subprocess

ffmpeg = os.environ["FFMPEG_BIN"]
ffprobe = os.environ["FFPROBE_BIN"]
system_prefixes = ("/System/", "/usr/lib/")

def deps(path: str) -> list[str]:
    try:
        out = subprocess.check_output(["otool", "-L", path], text=True)
    except subprocess.CalledProcessError:
        return []
    lines = out.splitlines()[1:]
    result = []
    for line in lines:
        dep = line.strip().split(" ")[0]
        if not dep or dep.startswith("@"):
            continue
        if dep.startswith(system_prefixes):
            continue
        result.append(dep)
    return result

seen: set[str] = set()
queue = [ffmpeg, ffprobe]
while queue:
    item = queue.pop(0)
    for dep in deps(item):
        if dep not in seen:
            seen.add(dep)
            queue.append(dep)

for dep in sorted(seen):
    print(dep)
PY
)"

BUNDLED_LIBS=()
if [[ -n "$DEPS" ]]; then
  echo "Bundling external dylibs:"
  while IFS= read -r dep; do
    [[ -z "$dep" ]] && continue
    base="$(basename "$dep")"
    echo "  $base <- $dep"
    cp -L "$dep" "$BINARIES_DIR/$base"
    chmod u+w "$BINARIES_DIR/$base"
    BUNDLED_LIBS+=("$base")
  done <<< "$DEPS"
fi

rewrite_install_names() {
  local file="$1"
  while IFS= read -r dep; do
    local base
    base="$(basename "$dep")"
    for lib in "${BUNDLED_LIBS[@]}"; do
      if [[ "$base" == "$lib" ]]; then
        install_name_tool -change "$dep" "@loader_path/$lib" "$file"
      fi
    done
  done < <(otool -L "$file" | awk 'NR>1 { print $1 }')
}

if ((${#BUNDLED_LIBS[@]})); then
  for lib in "${BUNDLED_LIBS[@]}"; do
    install_name_tool -id "@loader_path/$lib" "$BINARIES_DIR/$lib"
  done
  for file in "$BINARIES_DIR/ffmpeg-$SUFFIX" "$BINARIES_DIR/ffprobe-$SUFFIX"; do
    rewrite_install_names "$file"
  done
  for lib in "${BUNDLED_LIBS[@]}"; do
    rewrite_install_names "$BINARIES_DIR/$lib"
  done
  if command -v codesign >/dev/null 2>&1; then
    if [[ -n "$CODESIGN_IDENTITY" ]]; then
      echo "Signing bundled dylibs and binaries with identity: $CODESIGN_IDENTITY"
      for lib in "${BUNDLED_LIBS[@]}"; do
        codesign --force --sign "$CODESIGN_IDENTITY" "$BINARIES_DIR/$lib"
      done
      codesign --force --sign "$CODESIGN_IDENTITY" "$BINARIES_DIR/ffmpeg-$SUFFIX"
      codesign --force --sign "$CODESIGN_IDENTITY" "$BINARIES_DIR/ffprobe-$SUFFIX"
    else
      echo "Ad-hoc signing bundled dylibs and binaries (set TINY_VID_CODESIGN_IDENTITY for release signing)"
      for lib in "${BUNDLED_LIBS[@]}"; do
        codesign --force --sign - "$BINARIES_DIR/$lib"
      done
      codesign --force --sign - "$BINARIES_DIR/ffmpeg-$SUFFIX"
      codesign --force --sign - "$BINARIES_DIR/ffprobe-$SUFFIX"
    fi
  fi
fi

chmod +x "$BINARIES_DIR/ffmpeg-$SUFFIX" "$BINARIES_DIR/ffprobe-$SUFFIX"

echo "Done. Binaries at:"
echo "  $BINARIES_DIR/ffmpeg-$SUFFIX"
echo "  $BINARIES_DIR/ffprobe-$SUFFIX"
echo "Run yarn build:standalone to use these binaries."
