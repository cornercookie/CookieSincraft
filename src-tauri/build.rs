fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "select_image",
            "select_multiple_images",
            "select_save_path",
            "select_output_folder",
            "load_image",
            "load_image_from_base64",
            "check_path_exists",
            "upscale",
            "cancel_job",
            "export_image",
            "convert_image",
            "read_settings",
            "write_settings",
            "add_recent_file",
            "batch_upscale",
        ]),
    ))
    .expect("failed to build tauri app");
}
