fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(&[
                "select_image",
                "select_save_path",
                "load_image",
                "upscale",
                "cancel_job",
                "export_image",
                "convert_image",
            ]),
        ),
    )
    .expect("failed to build tauri app");
}