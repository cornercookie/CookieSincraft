//! Persistent settings for CookieSincraft.
//!
//! Stores user preferences (backend, algorithm, quality, output folder,
//! keep-aspect-ratio, filename template, recent files) in `settings.json`
//! under the Tauri app config directory. Written atomically (tmp file +
//! rename) to avoid partial/corrupt files on crash.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tracing::warn;

/// Persisted user settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub backend: String,
    pub algorithm: String,
    pub quality: u8,
    pub output_folder: Option<String>,
    pub keep_aspect_ratio: bool,
    pub filename_template: String,
    pub recent_files: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            backend: "cpu".to_string(),
            algorithm: "Lanczos3".to_string(),
            quality: 85,
            output_folder: None,
            keep_aspect_ratio: false,
            filename_template: "{name}_upscaled_{scale}x.{ext}".to_string(),
            recent_files: Vec::new(),
        }
    }
}

/// Maximum number of recent files to retain (newest first, evict tail).
const MAX_RECENT_FILES: usize = 5;

/// Resolve the settings file path under the Tauri app config directory.
/// Path is constructed by the backend only; the frontend never supplies it.
pub fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("failed to resolve app config dir: {e}"))?;
    std::fs::create_dir_all(&config_dir).map_err(|e| format!("failed to create config dir: {e}"))?;
    Ok(config_dir.join("settings.json"))
}

/// Atomically write settings to `path` via a `.tmp` file then rename.
fn atomic_write_json(path: &PathBuf, settings: &Settings) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    let content =
        serde_json::to_string_pretty(settings).map_err(|e| format!("serialize settings failed: {e}"))?;
    std::fs::write(&tmp, content).map_err(|e| format!("write settings tmp failed: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("rename settings failed: {e}"))?;
    Ok(())
}

/// Read settings from `path`. On missing or corrupt file, return defaults
/// (never propagate a corrupt file as a hard error).
fn read_settings_file(path: &PathBuf) -> Settings {
    match std::fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<Settings>(&content) {
            Ok(s) => s,
            Err(e) => {
                warn!("settings file corrupt, using defaults: {e}");
                Settings::default()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Settings::default(),
        Err(e) => {
            warn!("read settings failed, using defaults: {e}");
            Settings::default()
        }
    }
}

/// Append `path` to recent files (dedupe, newest first, cap at 5).
fn push_recent_file(settings: &mut Settings, path: &str) {
    settings.recent_files.retain(|p| p != path);
    settings.recent_files.insert(0, path.to_string());
    settings.recent_files.truncate(MAX_RECENT_FILES);
}

/// Read all settings. Corrupt/missing file falls back to defaults.
#[tauri::command]
pub fn read_settings(app: AppHandle) -> Result<Settings, String> {
    let path = settings_path(&app)?;
    Ok(read_settings_file(&path))
}

/// Write all settings. Path is backend-constructed; frontend supplies only
/// the settings payload, never a file path.
#[tauri::command]
pub fn write_settings(
    settings: Settings,
    app: AppHandle,
) -> Result<(), String> {
    let path = settings_path(&app)?;
    atomic_write_json(&path, &settings)
}

/// Add a file to the recent-files list and persist.
#[tauri::command]
pub fn add_recent_file(
    path: String,
    app: AppHandle,
) -> Result<Settings, String> {
    let settings_path = settings_path(&app)?;
    let mut settings = read_settings_file(&settings_path);
    push_recent_file(&mut settings, &path);
    atomic_write_json(&settings_path, &settings)?;
    Ok(settings)
}
