# Third-party licenses

Tiny Vid is licensed under the [MIT License](LICENSE).

Some build variants bundle FFmpeg. The bundled FFmpeg is a separate work with its own license:

| Mode | Profile | FFmpeg source | License |
| ---- | ------- | ------------- | ------- |
| **standalone** | **gpl** (macOS) | Self-built from FFmpeg source | GPL v2+ |
| **standalone** | **gpl** (Windows) | BtbN GPL build | GPL v2+ |
| **standalone** | **lgpl-vt** (macOS) | Custom build | LGPL v2.1+ |
| **system** | n/a | None (system) | N/A |

When distributing **standalone** builds, you must comply with the selected FFmpeg profile license (provide source code, license text, and notices as required). The **system** mode does not include FFmpeg.

- **macOS standalone + gpl**: FFmpeg is built from source via `scripts/build-ffmpeg-standalone-macos.sh`.
- **macOS standalone + lgpl-vt**: FFmpeg is built from source via `scripts/build-ffmpeg-lgpl.sh`.
- **BtbN FFmpeg** (Windows standalone): https://github.com/BtbN/FFmpeg-Builds
- **FFmpeg project**: https://ffmpeg.org/
