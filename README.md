# Tiny Vid

Tiny Vid is a desktop app for compressing video with real time preview on macOS, Windows, and Linux using ffmpeg.

Supports H.264, H.265, VP9, and AV1. Web version: [handy.tools](https://handy.tools)

![image](https://github.com/user-attachments/assets/7faa0c2b-320e-45ef-b556-fa35b87142a7)

## Build

Run from repo root. Installer artifacts go to `releases/electron/`.

### Build targets

| Build command                | Profile   | Platform   | FFmpeg source                           |
| ---------------------------- | --------- | ---------- | --------------------------------------- |
| `yarn build`                 | n/a       | Any        | System FFmpeg                           |
| `yarn build:standalone`      | `gpl`     | macOS      | Built from source (local build script)  |
| `yarn build:standalone`      | `gpl`     | Windows    | BtbN GPL build                          |
| `yarn build:standalone:lgpl` | `lgpl-vt` | macOS only | Built from source (LGPL + VideoToolbox) |

FFmpeg requirements:

- `system` mode (`yarn build`, `yarn dev`) uses your local FFmpeg from `PATH`.
- `standalone` mode bundles FFmpeg for the selected profile.

Prerequisites: Node.js 24+, Rust (for the native sidecar), and system FFmpeg on `PATH` for dev/build in system mode.

## Run in dev mode

- `yarn dev` (`system`)
- `yarn tv dev --mode standalone --profile gpl` (`standalone` + `gpl`)
- `yarn tv dev --mode standalone --profile lgpl-vt` (`standalone` + `lgpl-vt`, requires VideoToolbox)

## `tv` CLI

`yarn tv` is the lower-level script runner behind the build/dev/test wrappers (`scripts/tv.ts`).

Use `yarn tv --help` or `yarn tv <command> --help` for command-level help.

Common uses:

| Task                                | Command                                  |
| ----------------------------------- | ---------------------------------------- |
| Run every supported test set        | `yarn tv test matrix`                    |
| Prepare FFmpeg binaries by profile  | `yarn tv ffmpeg prepare --profile gpl`   |
| Build FFmpeg from source by profile | `yarn tv ffmpeg build --profile lgpl-vt` |

Useful flags:

- `--dry-run` prints commands without executing them.
- `--verbose` prints extra output.
- `--mode system|standalone` applies to `build`, `dev`, and `test`.
- `--profile gpl|lgpl-vt` is required when `--mode standalone`.
- `--suite` applies to `tv test`; `discovery` is system-only and `integration-contract` is standalone-only.

## License

MIT for this project. See [LICENSE](LICENSE). Some build variants bundle GPL-licensed or LGPL-licensed FFmpeg; see [THIRD_PARTY.md](THIRD_PARTY.md).
