use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EyePairSettings {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default = "default_interval")]
    pub interval: u64,
    #[serde(default = "default_corner")]
    pub corner: String,
    #[serde(default = "default_placement_mode")]
    pub placement_mode: String,
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
    #[serde(default = "default_display_duration")]
    pub display_duration: u64,
    #[serde(default = "default_style")]
    pub style: String,
}

fn default_interval() -> u64 {
    5000
}

fn default_corner() -> String {
    "bottom-right".to_string()
}

fn default_placement_mode() -> String {
    "preset".to_string()
}

fn default_display_duration() -> u64 {
    1000
}

fn default_style() -> String {
    "classic".to_string()
}

impl Default for EyePairSettings {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            hidden: false,
            interval: default_interval(),
            corner: default_corner(),
            placement_mode: default_placement_mode(),
            x: None,
            y: None,
            display_duration: default_display_duration(),
            style: default_style(),
        }
    }
}

impl EyePairSettings {
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            hidden: false,
            interval: default_interval(),
            corner: default_corner(),
            placement_mode: default_placement_mode(),
            x: None,
            y: None,
            display_duration: default_display_duration(),
            style: default_style(),
        }
    }

    pub fn normalize(&mut self) {
        if self.interval == 0 {
            self.interval = default_interval();
        }
        if self.display_duration == 0 {
            self.display_duration = default_display_duration();
        }
        if self.corner.is_empty() {
            self.corner = default_corner();
        }
        if self.placement_mode.is_empty() {
            self.placement_mode = default_placement_mode();
        }
        if self.style.is_empty() {
            self.style = default_style();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub selected_pair_id: String,
    pub pairs: Vec<EyePairSettings>,
    #[serde(default)]
    pub start_on_boot: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let default_pair = EyePairSettings::new("pair-1".to_string(), "Main Pair".to_string());
        Self {
            selected_pair_id: default_pair.id.clone(),
            pairs: vec![default_pair],
            start_on_boot: false,
        }
    }
}

impl Settings {
    pub fn normalize(&mut self) {
        if self.pairs.is_empty() {
            *self = Self::default();
            return;
        }

        for pair in &mut self.pairs {
            pair.normalize();
        }

        if !self
            .pairs
            .iter()
            .any(|pair| pair.id == self.selected_pair_id)
        {
            self.selected_pair_id = self.pairs[0].id.clone();
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacySettings {
    interval: u64,
    corner: String,
    display_duration: u64,
    style: String,
}

impl From<LegacySettings> for Settings {
    fn from(value: LegacySettings) -> Self {
        let mut settings = Settings::default();
        if let Some(pair) = settings.pairs.first_mut() {
            pair.interval = value.interval;
            pair.corner = value.corner;
            pair.display_duration = value.display_duration;
            pair.style = value.style;
            pair.hidden = false;
        }
        settings
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
    let data = match fs::read_to_string(&path) {
        Ok(data) => data,
        Err(_) => return Settings::default(),
    };

    if let Ok(mut settings) = serde_json::from_str::<Settings>(&data) {
        settings.normalize();
        return settings;
    }

    if let Ok(legacy) = serde_json::from_str::<LegacySettings>(&data) {
        return legacy.into();
    }

    Settings::default()
}

pub fn save(app: &tauri::AppHandle, settings: &Settings) {
    let path = settings_path(app);
    let json = serde_json::to_string_pretty(settings).expect("failed to serialize settings");
    fs::write(path, json).expect("failed to write settings");
}
