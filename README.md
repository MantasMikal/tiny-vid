# Tiny Vid

Cross-platform app for compressing and optimizing video files with support for H.264, H.265, VP9, and AV1 codecs. [Web version (WASM)](https://handy.tools)

![image](https://github.com/user-attachments/assets/7faa0c2b-320e-45ef-b556-fa35b87142a7)

## Build variants (FFmpeg)

| Mode | Profile | Platform | FFmpeg source |
| ---- | ------- | -------- | ------------- |
| system | n/a | Any | System FFmpeg |
| standalone | gpl | macOS | Self-built from source |
| standalone | gpl | Windows | BtbN GPL build |
| standalone | lgpl-vt | macOS only | Custom LGPL + VideoToolbox build |

- **system**: no FFmpeg in bundle; app uses system FFmpeg.
- **gpl**: bundles GPL FFmpeg (macOS self-built, Windows BtbN).
- **lgpl-vt**: custom LGPL + VideoToolbox (macOS only).

### How to build

From the repo root, run the command for the variant you want. Installers go to **releases/\<platform>\/**.

| Command | Description |
| ------- | ----------- |
| `yarn build` | system mode (no bundled FFmpeg). |
| `yarn build:standalone` | standalone + gpl. |
| `yarn build:standalone:lgpl` | standalone + lgpl-vt (macOS only). |

Prerequisites:
- macOS standalone gpl: `yarn build-ffmpeg-standalone` (requires `brew install x264 x265 libvpx opus svt-av1 dav1d pkg-config`).
- macOS standalone lgpl-vt: `yarn build-ffmpeg-standalone:lgpl`.
- Windows standalone gpl: `yarn prepare-ffmpeg` downloads BtbN (cached in `%LOCALAPPDATA%\\tiny-vid\\cache\\ffmpeg`; macOS/Linux `~/.cache/tiny-vid/ffmpeg`).
- `prepare-ffmpeg` defaults to `--profile gpl`; use `yarn prepare-ffmpeg:lgpl` for lgpl-vt.
- Linux: system mode only. Build on Linux with `yarn build`; `.deb` output in `releases/linux/`. Install Tauri prerequisites first:
  - **Ubuntu/Debian:** `sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev ca-certificates pkg-config`
  - **Other distros:** [Tauri v2 Linux prerequisites](https://v2.tauri.app/start/prerequisites/)

Orchestration uses `yarn tv ...` (script runner in `scripts/tv.ts`):
- `yarn tv build --mode standalone --profile gpl`
- `yarn tv ffmpeg prepare --profile gpl`
- `yarn tv ffmpeg build --profile lgpl-vt`
- Add `--dry-run` to preview without executing (e.g. `yarn tv build --dry-run`); `--verbose` for extra output.

### Dev commands

- `yarn dev` — system FFmpeg.
- `yarn dev:standalone` — standalone + gpl.
- `yarn dev:standalone:lgpl` — standalone + lgpl-vt (requires VideoToolbox).

## Testing

From project root:

- `yarn test` — unit + command tests (integration skipped by default).
- `yarn test:integration` — FFmpeg integration (needs libx264, libx265, libsvtav1).
- `yarn test:discovery` — discovery tests (env/cache isolation).
- **standalone gpl**: `yarn test:standalone`, `yarn test:standalone:ffmpeg`.
- **standalone lgpl-vt**: `yarn test:standalone:lgpl`, `yarn test:standalone:lgpl:ffmpeg` (fails if VideoToolbox unavailable).

Tests: unit tests in each module; Tauri commands in `commands_tests.rs`; FFmpeg integration in `integration_tests.rs`.

## License

Tiny Vid is MIT licensed. See [LICENSE](LICENSE). Some build variants bundle FFmpeg under GPL or LGPL—see [THIRD_PARTY.md](THIRD_PARTY.md) for details.
