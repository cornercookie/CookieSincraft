#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

mod commands;
mod converter;
mod cpu;
mod gpu;
mod settings;
mod upscaler;
#[cfg(all(test, windows))]
mod batch_test;

use crate::commands::{AppState, add_recent_file, batch_upscale, cancel_job, check_path_exists, convert_image, export_image, load_image, load_image_from_base64, select_image, select_multiple_images, select_output_folder, select_save_path, upscale};
use crate::settings::{read_settings, write_settings};

fn main() {
    tauri::Builder::default()
        .manage(AppState::new())
        .setup(|app| {
            #[cfg(debug_assertions)]
            app.get_webview_window("main").unwrap().open_devtools();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            select_image,
            select_multiple_images,
            select_save_path,
            select_output_folder,
            load_image,
            load_image_from_base64,
            upscale,
            cancel_job,
            export_image,
            convert_image,
            read_settings,
            write_settings,
            add_recent_file,
            batch_upscale,
            check_path_exists,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}