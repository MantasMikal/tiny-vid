# Tiny Vid

A video compressor

https://handy.tools but as an app using native FFMPEG for speed

![image](https://github.com/user-attachments/assets/7faa0c2b-320e-45ef-b556-fa35b87142a7)

## Testing

From the `src-tauri` directory run `cargo test`. Unit tests live in each module (e.g. `error`, `ffmpeg/builder`); Tauri command tests are in `commands_tests.rs`; the FFmpeg integration test is in `integration_tests.rs` and is ignored by default. To run it (requires FFmpeg on the system): `cargo test ffmpeg_transcode_integration -- --ignored`.

