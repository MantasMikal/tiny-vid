# Third-party licenses

Tiny Vid is licensed under the [MIT License](LICENSE).

Some build variants bundle FFmpeg. The bundled FFmpeg is a separate work with its own license:

| Build variant | FFmpeg source   | License      |
| ------------- | --------------- | ------------ |
| **full** (macOS) | Self-built from FFmpeg source (GPL) | GPL v2+ |
| **full** (Windows) | BtbN GPL build  | GPL v2+      |
| **lgpl-macos**| Custom build    | LGPL v2.1+   |
| **bare**      | None (system)   | N/A          |

When distributing the **full** or **lgpl-macos** builds, you must comply with the applicable FFmpeg license (provide source code, license text, and notices as required). The **bare** build does not include FFmpeg.

- **macOS full**: FFmpeg is built from source via `scripts/build-ffmpeg-full-macos.sh` (FFmpeg from https://git.ffmpeg.org/ffmpeg.git).
- **BtbN FFmpeg** (Windows full): https://github.com/BtbN/FFmpeg-Builds
- **FFmpeg project**: https://ffmpeg.org/
