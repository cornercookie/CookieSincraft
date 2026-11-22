#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

mod commands;
mod converter;
mod cpu;
mod gpu;
mod upscaler;

use crate::commands::*;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(debug_assertions)] // only enable DevTools in debug build
            app.get_webview_window("main").unwrap().open_devtools();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
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
