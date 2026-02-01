fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(&[
                "ffmpeg_transcode_to_temp",
                "ffmpeg_preview",
                "ffmpeg_terminate",
                "get_file_size",
                "get_video_metadata",
                "move_compressed_file",
                "cleanup_temp_file",
            ]),
        ),
    )
    .expect("failed to run tauri build");
}
