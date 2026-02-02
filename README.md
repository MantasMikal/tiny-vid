# Tiny Vid

Video compressor app using native FFmpeg. [Web version (WASM)](https://handy.tools)



![image](https://github.com/user-attachments/assets/7faa0c2b-320e-45ef-b556-fa35b87142a7)

## Build variants (FFmpeg bundling)

| Variant     | Platform       | FFmpeg source  |
| ----------- | -------------- | -------------- |
| standalone  | macOS          | Self-built from source (GPL) |
| standalone | Windows        | BtbN GPL       |
| lgpl-macos | macOS only     | Custom LGPL+VT |
| default    | Any            | System         |

- **Standalone (macOS)** – Bundles GPL FFmpeg (libx264, libx265, etc.). Build from source: run `yarn build-ffmpeg-standalone-macos` once, then `yarn build:standalone`.
- **Standalone (Windows)** – Bundles BtbN GPL FFmpeg (libx264, libx265, libsvtav1).
- **lgpl-macos (macOS App Store)** – Custom LGPL FFmpeg with VideoToolbox only (H.264/HEVC, MP4). Requires building FFmpeg first.
- **Default** – No FFmpeg in the bundle; app uses system FFmpeg (e.g. `apt install ffmpeg` on Linux).

### How to build

From the repo root, run the script for the variant you want. Installers go to **releases/<platform>/**.

| Script | Description |
| ------ | ----------- |
| `yarn build` | No bundled FFmpeg; uses system FFmpeg. Works on macOS, Windows, Linux (Linux requires Docker). |
| `yarn build:standalone` | Bundles FFmpeg. macOS: build from source first (`yarn build-ffmpeg-standalone-macos`); Windows: BtbN download. |
| `yarn build:lgpl-macos` | macOS App Store build (custom LGPL FFmpeg). Run `yarn build-ffmpeg-lgpl-macos` once first. |

To build by platform script instead: `./scripts/build-macos.sh [standalone|lgpl]`, `./scripts/build-linux.sh`, or `./scripts/build-windows.sh [standalone]`.

macOS standalone requires building FFmpeg from source first: `yarn build-ffmpeg-standalone-macos` (requires `brew install x264 x265 libvpx opus pkg-config`). Windows standalone downloads BtbN FFmpeg (cached in `~/.cache/tiny-vid/ffmpeg`; checksums verified). Binaries go to `src-tauri/binaries/` (gitignored). lgpl-macos expects custom FFmpeg from `yarn build-ffmpeg-lgpl-macos`.

### Dev commands

Run the app in development with a specific variant (uses system FFmpeg or `FFMPEG_PATH`; no prepare-ffmpeg needed):

- **Default**: `yarn tauri dev` (default config).
- **dev:standalone**: `yarn dev:standalone` — standalone config (bundled FFmpeg).
- **dev:lgpl-macos**: `yarn dev:lgpl-macos` — lgpl-macos variant (App Store build in dev).

## Testing

Run from project root:

- **Default**  
  - `yarn test` — unit and command tests; integration tests are `#[ignore]`.  
  - `yarn test:integration` — integration tests (needs FFmpeg with libx264, libx265, libsvtav1).  
  - `yarn test:discovery` — discovery tests (env/cache isolation).
- **lgpl-macos**  
  - `yarn test:lgpl-macos` (alias: `yarn test:lgpl`) — unit and command tests (including get_build_variant and VideoToolbox builder tests).  
  - `yarn test:lgpl-macos:integration` (alias: `yarn test:lgpl:ffmpeg`) — integration tests with VideoToolbox (requires `yarn build-ffmpeg-lgpl-macos` first; macOS only, VideoToolbox may fail in headless/CI).

Unit tests live in each module (e.g. `error`, `ffmpeg/builder`); Tauri command tests are in `commands_tests.rs`; the FFmpeg integration test is in `integration_tests.rs` and is ignored by default.

## License

Tiny Vid is MIT licensed. See [LICENSE](LICENSE). Some build variants bundle FFmpeg under GPL or LGPL—see [THIRD_PARTY.md](THIRD_PARTY.md) for details.
