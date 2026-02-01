# Tiny Vid

Video compressor app using native FFmpeg. [Web version (WASM)](https://handy.tools)

![image](https://github.com/user-attachments/assets/7faa0c2b-320e-45ef-b556-fa35b87142a7)

## Build variants (FFmpeg bundling)

| Variant    | Platform       | FFmpeg source  |
| ---------- | -------------- | -------------- |
| full       | macOS, Windows | BtbN GPL       |
| lgpl-macos | macOS only     | Custom LGPL+VT |
| bare       | Any            | System         |

- **Full (macOS / Windows)** – Bundles BtbN GPL FFmpeg (libx264, libx265, libsvtav1).
- **lgpl-macos (macOS App Store)** – Custom LGPL FFmpeg with VideoToolbox only (H.264/HEVC, MP4). Requires building FFmpeg first.
- **Bare** – No FFmpeg in the bundle; app uses system FFmpeg (e.g. `apt install ffmpeg` on Linux).

### How to build

From the repo root, run the script for the variant you want. Installers go to **releases/<platform>/**.

| Script | Description |
| ------ | ----------- |
| `yarn build:bare` | No bundled FFmpeg; uses system FFmpeg. Works on macOS, Windows, Linux (Linux requires Docker). |
| `yarn build:full` | Bundles BtbN FFmpeg. macOS and Windows only. |
| `yarn build:lgpl-macos` | macOS App Store build (custom LGPL FFmpeg). Run `yarn build-ffmpeg-lgpl-macos` once first. |

To build by platform script instead: `./scripts/build-macos.sh [full|bare|lgpl]`, `./scripts/build-linux.sh`, or `./scripts/build-windows.sh [full|bare]`.

BtbN downloads are cached in `~/.cache/tiny-vid/ffmpeg` (or `TINY_VID_FFMPEG_CACHE`). Checksums are verified when BtbN provides `checksums.sha256`. Binaries go to `src-tauri/binaries/` (gitignored). lgpl-macos build expects custom FFmpeg there from `yarn build-ffmpeg-lgpl-macos`.

### Dev commands

Run the app in development with a specific variant (uses system FFmpeg or `FFMPEG_PATH`; no prepare-ffmpeg needed):

- **Default**: `yarn tauri dev` (full config).
- **dev:full**: `yarn dev:full` — explicit full config.
- **dev:lgpl-macos**: `yarn dev:lgpl-macos` — lgpl-macos variant (App Store build in dev).
- **dev:bare**: `yarn dev:bare` — bare config (no externalBin).

## Testing

Run from project root:

- **Default (full)**  
  - `yarn test` — unit and command tests; integration tests are `#[ignore]`.  
  - `yarn test:integration` — integration tests (needs FFmpeg with libx264, libx265, libsvtav1).  
  - `yarn test:discovery` — discovery tests (env/cache isolation).  
  - `yarn test:bundled-smoke` — smoke test for bundled ffmpeg (run after `yarn build:full`).
- **lgpl-macos**  
  - `yarn test:lgpl-macos` — unit and command tests (including get_build_variant and VideoToolbox builder tests).  
  - `yarn test:lgpl-macos:integration` — integration tests with VideoToolbox (requires `yarn build-ffmpeg-lgpl-macos` first; macOS only, VideoToolbox may fail in headless/CI).

Unit tests live in each module (e.g. `error`, `ffmpeg/builder`); Tauri command tests are in `commands_tests.rs`; the FFmpeg integration test is in `integration_tests.rs` and is ignored by default.

