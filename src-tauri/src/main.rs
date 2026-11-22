#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

mod commands;
mod converter;
mod cpu;
mod gpu;
mod upscaler;

use crate::commands::{AppState, cancel_job, convert_image, export_image, load_image, select_image, select_save_path, upscale};

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
            select_save_path,
            load_image,
            upscale,
            cancel_job,
            export_image,
            convert_image
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}