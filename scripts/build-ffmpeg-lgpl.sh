#!/usr/bin/env bash
# Build FFmpeg from source with macOS LGPL profile:
# - VideoToolbox H.264/H.265
# - software VP9 (libvpx) and AV1 (libsvtav1)
# - WebM/Matroska/MP4 muxing
# Output: native/binaries/standalone-lgpl-vt/ffmpeg-<target>, ffprobe-<target>, required dylibs
#
# Dependencies (Homebrew):
#   brew install libvpx opus svt-av1 pkg-config
# Optional for x86_64-native builds:
#   brew install nasm

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARIES_DIR="$ROOT/native/binaries/standalone-lgpl-vt"
BUILD_DIR="${FFMPEG_BUILD_DIR:-/tmp/ffmpeg-lgpl-build}"
# FFmpeg release branch (e.g. 8.0 -> release/8.0). See https://git.ffmpeg.org/ffmpeg.git
FFMPEG_VERSION="${FFMPEG_VERSION:-8.0}"
TARGET_TRIPLE="${TARGET_TRIPLE:-$(rustc --print host-tuple 2>/dev/null || echo "aarch64-apple-darwin")}"
CORE_LGPL_DYLIBS=(
  "libavcodec"
  "libavdevice"
  "libavfilter"
  "libavformat"
  "libavutil"
  "libswresample"
  "libswscale"
)
CODESIGN_IDENTITY="${TINY_VID_CODESIGN_IDENTITY:-${CODESIGN_IDENTITY:-}}"

echo "Building FFmpeg LGPL (VT + VP9 + AV1 + WebM) for $TARGET_TRIPLE (release/$FFMPEG_VERSION)"

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
  --enable-libvpx \
  --enable-libsvtav1 \
  --enable-libopus \
  \
  --enable-encoder=h264_videotoolbox \
  --enable-encoder=hevc_videotoolbox \
  --enable-encoder=aac_at \
  --enable-encoder=libvpx-vp9 \
  --enable-encoder=libsvtav1 \
  --enable-encoder=libopus \
  \
  --enable-muxer=mp4 \
  --enable-muxer=matroska \
  --enable-muxer=webm \
  --enable-demuxer=mov \
  --enable-demuxer=matroska \
  --enable-demuxer=srt \
  \
  --enable-protocol=file \
  \
  --disable-libx264 \
  --disable-libx265 \
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

CORE_DYLIB_PATHS=()
for lib in "${CORE_LGPL_DYLIBS[@]}"; do
  src="$BUILD_DIR/install/lib/${lib}.dylib"
  if [[ ! -f "$src" ]]; then
    echo "error: missing required dylib: $src" >&2
    exit 1
  fi
  out="$BINARIES_DIR/${lib}.dylib"
  cp -L "$src" "$out"
  chmod u+w "$out"
  CORE_DYLIB_PATHS+=("$out")
done

is_system_dependency() {
  local dep="$1"
  [[ "$dep" == /System/* || "$dep" == /usr/lib/* ]]
}

resolve_dependency_source() {
  local dep="$1"
  local owner_file="$2"
  local dep_name
  dep_name="$(basename "$dep")"

  if [[ "$dep" == @loader_path/* ]]; then
    local bundled="$BINARIES_DIR/$dep_name"
    if [[ -f "$bundled" ]]; then
      echo "$bundled"
      return 0
    fi
  fi

  if [[ "$dep" == @rpath/* ]]; then
    local owner_dir exe_dir
    owner_dir="$(dirname "$owner_file")"
    exe_dir="$BINARIES_DIR"

    while IFS= read -r rpath; do
      [[ -z "$rpath" ]] && continue
      local resolved_rpath candidate
      resolved_rpath="$rpath"
      resolved_rpath="${resolved_rpath//@loader_path/$owner_dir}"
      resolved_rpath="${resolved_rpath//@executable_path/$exe_dir}"
      candidate="$resolved_rpath/$dep_name"
      if [[ -f "$candidate" ]]; then
        echo "$candidate"
        return 0
      fi
    done < <(
      otool -l "$owner_file" \
        | awk '
            $1=="cmd" && $2=="LC_RPATH" { in_rpath=1; next }
            in_rpath && $1=="path" { print $2; in_rpath=0 }
          '
    )

    local install_candidate="$BUILD_DIR/install/lib/$dep_name"
    if [[ -f "$install_candidate" ]]; then
      echo "$install_candidate"
      return 0
    fi
  fi

  if [[ -f "$dep" ]]; then
    echo "$dep"
    return 0
  fi

  return 1
}

collect_runtime_dependencies() {
  local queue=("$@")
  local processed_file
  processed_file="$(mktemp)"
  local index=0

  while [[ $index -lt ${#queue[@]} ]]; do
    local file="${queue[$index]}"
    index=$((index + 1))

    if grep -Fxq "$file" "$processed_file"; then
      continue
    fi
    echo "$file" >> "$processed_file"

    while IFS= read -r dep; do
      [[ -z "$dep" ]] && continue
      if is_system_dependency "$dep"; then
        continue
      fi

      local dep_name dep_target dep_source
      dep_name="$(basename "$dep")"
      dep_target="$BINARIES_DIR/$dep_name"

      if [[ ! -f "$dep_target" ]]; then
        dep_source="$(resolve_dependency_source "$dep" "$file")" || {
          echo "error: unable to resolve dependency '$dep' referenced by '$file'" >&2
          rm -f "$processed_file"
          exit 1
        }
        cp -L "$dep_source" "$dep_target"
      fi

      if [[ "$dep_target" == *.dylib ]]; then
        queue+=("$dep_target")
      fi
    done < <(otool -L "$file" | awk 'NR>1 { print $1 }')
  done

  rm -f "$processed_file"
}

rewrite_install_names() {
  local file="$1"
  while IFS= read -r dep; do
    [[ -z "$dep" ]] && continue
    if is_system_dependency "$dep"; then
      continue
    fi

    local dep_name bundled desired
    dep_name="$(basename "$dep")"
    bundled="$BINARIES_DIR/$dep_name"
    desired="@loader_path/$dep_name"

    if [[ -f "$bundled" && "$dep" != "$desired" ]]; then
      install_name_tool -change "$dep" "$desired" "$file"
    fi
  done < <(otool -L "$file" | awk 'NR>1 { print $1 }')
}

verify_install_names() {
  local file="$1"
  local unresolved
  unresolved="$(
    otool -L "$file" \
      | awk 'NR>1 { print $1 }' \
      | grep -E "^${BUILD_DIR}/install/lib/|^@rpath/|^/opt/homebrew/|^/usr/local/|^/opt/local/" \
      || true
  )"
  if [[ -n "$unresolved" ]]; then
    echo "error: unresolved shared library references in $file" >&2
    echo "$unresolved" >&2
    exit 1
  fi
}

collect_runtime_dependencies \
  "$BINARIES_DIR/ffmpeg-$SUFFIX" \
  "$BINARIES_DIR/ffprobe-$SUFFIX" \
  "${CORE_DYLIB_PATHS[@]}"

shopt -s nullglob
for dylib in "$BINARIES_DIR"/*.dylib; do
  install_name_tool -id "@loader_path/$(basename "$dylib")" "$dylib"
done

for file in "$BINARIES_DIR/ffmpeg-$SUFFIX" "$BINARIES_DIR/ffprobe-$SUFFIX" "$BINARIES_DIR"/*.dylib; do
  rewrite_install_names "$file"
  verify_install_names "$file"
done
shopt -u nullglob

# install_name_tool mutates Mach-O load commands. Re-sign all bundled artifacts afterwards,
# otherwise macOS may kill ffmpeg/ffprobe at launch with SIGKILL and no stderr.
shopt -s nullglob
for dylib in "$BINARIES_DIR"/*.dylib; do
  codesign --force --sign - "$dylib"
done
shopt -u nullglob
codesign --force --sign - "$BINARIES_DIR/ffprobe-$SUFFIX"
codesign --force --sign - "$BINARIES_DIR/ffmpeg-$SUFFIX"

if command -v codesign >/dev/null 2>&1; then
  if [[ -n "$CODESIGN_IDENTITY" ]]; then
    echo "Signing LGPL binaries with identity: $CODESIGN_IDENTITY"
    shopt -s nullglob
    for dylib in "$BINARIES_DIR"/*.dylib; do
      codesign --force --sign "$CODESIGN_IDENTITY" "$dylib"
    done
    shopt -u nullglob
    codesign --force --sign "$CODESIGN_IDENTITY" "$BINARIES_DIR/ffmpeg-$SUFFIX"
    codesign --force --sign "$CODESIGN_IDENTITY" "$BINARIES_DIR/ffprobe-$SUFFIX"
  else
    echo "Ad-hoc signing LGPL binaries (set TINY_VID_CODESIGN_IDENTITY for release signing)"
    shopt -s nullglob
    for dylib in "$BINARIES_DIR"/*.dylib; do
      codesign --force --sign - "$dylib"
    done
    shopt -u nullglob
    codesign --force --sign - "$BINARIES_DIR/ffmpeg-$SUFFIX"
    codesign --force --sign - "$BINARIES_DIR/ffprobe-$SUFFIX"
  fi
fi

chmod +x "$BINARIES_DIR/ffmpeg-$SUFFIX" "$BINARIES_DIR/ffprobe-$SUFFIX"

echo "Done. Binaries at:"
echo "  $BINARIES_DIR/ffmpeg-$SUFFIX"
echo "  $BINARIES_DIR/ffprobe-$SUFFIX"
shopt -s nullglob
for dylib in "$BINARIES_DIR"/*.dylib; do
  echo "  $dylib"
done
shopt -u nullglob
