use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub interval: u64,
    pub corner: String,
    pub display_duration: u64,
    pub style: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            interval: 5000,
            corner: "bottom-right".to_string(),
            display_duration: 1000,
            style: "classic".to_string(),
        }
    }
}

fn settings_path(app: &tauri::AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .expect("failed to get app data dir");
    fs::create_dir_all(&dir).ok();
    dir.join("settings.json")
}

pub fn load(app: &tauri::AppHandle) -> Settings {
    let path = settings_path(app);
    match fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save(app: &tauri::AppHandle, settings: &Settings) {
    let path = settings_path(app);
    let json = serde_json::to_string_pretty(settings).expect("failed to serialize settings");
    fs::write(path, json).expect("failed to write settings");
}
