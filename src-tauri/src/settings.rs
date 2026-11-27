//! Persistent settings for CookieSincraft.
//!
//! Stores user preferences (backend, algorithm, quality, output folder,
//! keep-aspect-ratio, filename template) in `settings.json` under the Tauri
//! app config directory. Written atomically (tmp file + rename) to avoid
//! partial/corrupt files on crash.
//!
//! Recent files are kept in a *separate*, backend-only canonical store
//! (`recent.json`) guarded by a serialization lock. The `settings.json`
//! `recent_files` field is legacy display-only and is never trusted for
//! authorization. App-temp (drag-drop) paths are never elevated into the
//! canonical store across restart.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager};
use tracing::warn;

/// Serialization guard for concurrent recent-store writes. Prevents two
/// commands from read-modify-writing the shared `.tmp` file at once.
static RECENT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn recent_lock() -> &'static Mutex<()> {
    RECENT_LOCK.get_or_init(|| Mutex::new(()))
}

/// Persisted user settings (preferences only; `recent_files` is legacy
/// display-only).
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

/// Canonical, backend-only recent-file store (separate file from settings.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecentStore {
    pub recent_files: Vec<String>,
}

impl Default for RecentStore {
    fn default() -> Self {
        RecentStore {
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
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("failed to create config dir: {e}"))?;
    Ok(config_dir.join("settings.json"))
}

/// Resolve the canonical recent-store path (backend-only, separate from
/// settings.json).
pub fn recent_store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("failed to resolve app config dir: {e}"))?;
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("failed to create config dir: {e}"))?;
    Ok(config_dir.join("recent.json"))
}

/// True when `path` is an app-temp path that must never be persisted across
/// restart (drag-drop session files under `<temp>/cookiesincraft/...`).
fn is_app_temp_path(path: &str) -> bool {
    let p = path.replace('\\', "/");
    p.contains("/cookiesincraft/")
}

/// Atomically write JSON to `path` via a `.tmp` file then rename.
fn atomic_write_json(path: &PathBuf, settings: &impl Serialize) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("serialize settings failed: {e}"))?;
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

/// Read the canonical recent store. Missing/corrupt returns empty.
fn read_recent_store(path: &PathBuf) -> RecentStore {
    match std::fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<RecentStore>(&content) {
            Ok(s) => s,
            Err(e) => {
                warn!("recent store corrupt, using defaults: {e}");
                RecentStore::default()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => RecentStore::default(),
        Err(e) => {
            warn!("read recent store failed, using defaults: {e}");
            RecentStore::default()
        }
    }
}

/// Append `path` to recent files (dedupe, newest first, cap at 5).
fn push_recent_file(store: &mut RecentStore, path: &str) {
    store.recent_files.retain(|p| p != path);
    store.recent_files.insert(0, path.to_string());
    store.recent_files.truncate(MAX_RECENT_FILES);
}

/// Read all settings. `recent_files` is populated from the canonical backend
/// store for display; legacy `settings.json` recent_files is display-only.
#[tauri::command]
pub fn read_settings(app: AppHandle) -> Result<Settings, String> {
    let path = settings_path(&app)?;
    let mut settings = read_settings_file(&path);
    // Overlay canonical recent files for display (backend-only source of truth).
    if let Ok(store) = recent_store_path(&app) {
        if let Ok(content) = std::fs::read_to_string(&store) {
            if let Ok(store_data) = serde_json::from_str::<RecentStore>(&content) {
                settings.recent_files = store_data.recent_files;
            }
        }
    }
    Ok(settings)
}

/// Write all settings. Path is backend-constructed; frontend supplies only
/// the settings payload, never a file path. `recent_files` is preserved from
/// the existing file (legacy display-only) and is not injectable by the
/// webview.
#[tauri::command]
pub fn write_settings(mut settings: Settings, app: AppHandle) -> Result<(), String> {
    let path = settings_path(&app)?;
    // recent_files is owned by the canonical recent store. Do not let the
    // webview inject paths; preserve legacy display-only value.
    settings.recent_files = read_settings_file(&path).recent_files;
    atomic_write_json(&path, &settings)
}

/// True when `path` is recorded in the canonical recent store.
///
/// R1: trusts only the backend-owned canonical store, never legacy strings
/// from settings.json. Never returns true for app-temp (drag-drop) paths,
/// so dropped temps are never elevated across restart.
pub fn is_recent_file(app: &AppHandle, path: &str) -> bool {
    // Never elevate dropped temps across restart.
    if is_app_temp_path(path) {
        return false;
    }
    let Ok(store) = recent_store_path(app) else {
        return false;
    };
    let Ok(content) = std::fs::read_to_string(&store) else {
        return false;
    };
    let Ok(store_data) = serde_json::from_str::<RecentStore>(&content) else {
        return false;
    };
    store_data.recent_files.iter().any(|saved| saved == path)
}

/// Persist `path` to the canonical recent-files list (dedupe, newest first,
/// cap at 5). Serialized via `RECENT_LOCK` to avoid concurrent `.tmp` clobber.
///
/// Called by `commands::add_recent_file` after it authorizes `path`.
/// App-temp paths are rejected and never persisted across restart.
pub fn persist_recent_file(app: &AppHandle, path: &str) -> Result<Settings, String> {
    // Never persist app-temp paths across restart (drag-drop session files).
    if is_app_temp_path(path) {
        return Err("refusing to persist app-temp path to recent store".to_string());
    }

    let _guard = recent_lock().lock().unwrap();
    let store_path = recent_store_path(app)?;
    let mut store = read_recent_store(&store_path);
    push_recent_file(&mut store, path);
    atomic_write_json(&store_path, &store)?;

    // Return merged Settings for frontend display.
    let cfg_path = settings_path(app)?;
    let mut settings = read_settings_file(&cfg_path);
    settings.recent_files = store.recent_files.clone();
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// App-temp paths (drag-drop) are never recent.
    #[test]
    fn is_recent_file_rejects_app_temp_path() {
        // Just verify the helper identifies temp paths.
        assert!(is_app_temp_path(
            r"C:\Users\foo\AppData\Local\Temp\cookiesincraft\123_456\image.png"
        ));
        assert!(is_app_temp_path("/tmp/cookiesincraft/123_456/image.png"));
        assert!(!is_app_temp_path(r"C:\Users\foo\Pictures\image.png"));
        assert!(!is_app_temp_path("/home/foo/image.png"));
    }

    /// push_recent_file dedupes, keeps newest first, caps at 5.
    #[test]
    fn push_recent_file_dedupes_and_caps() {
        let mut store = RecentStore::default();
        for p in ["a.png", "b.png", "c.png", "d.png", "e.png", "f.png"] {
            push_recent_file(&mut store, p);
        }
        assert_eq!(store.recent_files.len(), 5);
        assert_eq!(store.recent_files[0], "f.png");
        assert!(!store.recent_files.contains(&"a.png".to_string()));

        // Re-adding an existing path moves it to front.
        push_recent_file(&mut store, "e.png");
        assert_eq!(store.recent_files[0], "e.png");
        assert_eq!(
            store.recent_files.iter().filter(|p| *p == "e.png").count(),
            1
        );
    }

    /// atomic_write_json produces valid JSON that round-trips.
    #[test]
    fn atomic_write_json_roundtrips() {
        let tmp = std::env::temp_dir().join("cookiesincraft_settings_test");
        std::fs::create_dir_all(&tmp).ok();
        let path = tmp.join("settings.json");
        let s = Settings::default();
        atomic_write_json(&path, &s).unwrap();
        let back: Settings =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(s.backend, back.backend);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
