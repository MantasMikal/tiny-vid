# Third-party licenses

Tiny Vid is licensed under the [MIT License](LICENSE).

Some build variants bundle FFmpeg. The bundled FFmpeg is a separate work with its own license:

| Build variant | FFmpeg source   | License      |
| ------------- | --------------- | ------------ |
| **standalone** (macOS) | Self-built from FFmpeg source (GPL) | GPL v2+ |
| **standalone** (Windows) | BtbN GPL build  | GPL v2+      |
| **lgpl-macos**| Custom build    | LGPL v2.1+   |
| **default**   | None (system)   | N/A          |

When distributing the **standalone** or **lgpl-macos** builds, you must comply with the applicable FFmpeg license (provide source code, license text, and notices as required). The **default** build does not include FFmpeg.

- **macOS standalone**: FFmpeg is built from source via `scripts/build-ffmpeg-standalone-macos.sh` (FFmpeg from https://git.ffmpeg.org/ffmpeg.git).
- **BtbN FFmpeg** (Windows standalone): https://github.com/BtbN/FFmpeg-Builds
- **FFmpeg project**: https://ffmpeg.org/
