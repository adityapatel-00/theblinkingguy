#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod settings;

use settings::{EyePairSettings, Settings};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Emitter, Manager, PhysicalPosition, Position, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;

const OVERLAY_WIDTH: f64 = 140.0;
const OVERLAY_HEIGHT: f64 = 80.0;
const DEFAULT_MARGIN: f64 = 20.0;
static NEXT_PAIR_ID: AtomicU64 = AtomicU64::new(1);

struct AppState {
    settings: Mutex<Settings>,
    arranging_pairs: Mutex<HashSet<String>>,
    hidden_before_arrange: Mutex<HashMap<String, bool>>,
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    mut new_settings: Settings,
) -> Result<Settings, String> {
    new_settings.normalize();

    {
        let mut current = state.settings.lock().unwrap();
        *current = new_settings.clone();
        settings::save(&app, &current);
    }

    apply_start_on_boot(&app, new_settings.start_on_boot)?;

    let app_clone = app.clone();
    let settings_clone = new_settings.clone();
    std::thread::spawn(move || {
        sync_overlay_windows(&app_clone, &settings_clone);
    });

    Ok(new_settings)
}

#[tauri::command]
fn create_eye_pair(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    name: Option<String>,
) -> Result<Settings, String> {
    let mut current = state.settings.lock().unwrap();
    let next_number = next_pair_number(&current);
    let id = format!(
        "pair-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_millis(),
        NEXT_PAIR_ID.fetch_add(1, Ordering::Relaxed)
    );

    let pair_name = name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("Pair {}", next_number));

    let pair = EyePairSettings::new(id.clone(), pair_name);
    current.selected_pair_id = id;
    current.pairs.push(pair);
    current.normalize();
    settings::save(&app, &current);

    let cloned = current.clone();
    drop(current);

    let app_clone = app.clone();
    let settings_clone = cloned.clone();
    std::thread::spawn(move || {
        sync_overlay_windows(&app_clone, &settings_clone);
    });

    Ok(cloned)
}

fn next_pair_number(settings: &Settings) -> usize {
    let mut index = 1;
    loop {
        let candidate = format!("Pair {}", index);
        if !settings
            .pairs
            .iter()
            .any(|pair| pair.name.eq_ignore_ascii_case(&candidate))
        {
            return index;
        }
        index += 1;
    }
}

fn apply_start_on_boot(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let autostart = app.autolaunch();
    if enabled {
        autostart.enable().map_err(|err| err.to_string())?;
    } else {
        autostart.disable().map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn delete_eye_pair(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    pair_id: String,
) -> Result<Settings, String> {
    let mut current = state.settings.lock().unwrap();
    if current.pairs.len() == 1 {
        return Err("At least one pair of eyes must remain.".to_string());
    }

    current.pairs.retain(|pair| pair.id != pair_id);
    current.normalize();
    settings::save(&app, &current);

    let cloned = current.clone();
    drop(current);

    state.arranging_pairs.lock().unwrap().remove(&pair_id);
    state.hidden_before_arrange.lock().unwrap().remove(&pair_id);
    if let Some(window) = app.get_webview_window(&overlay_label(&pair_id)) {
        window.close().ok();
    }
    sync_overlay_windows(&app, &cloned);

    Ok(cloned)
}

#[tauri::command]
fn start_arranging_pair(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    pair_id: String,
) -> Result<(), String> {
    let (was_hidden, settings) = {
        let mut current = state.settings.lock().unwrap();
        let pair = current
            .pairs
            .iter_mut()
            .find(|pair| pair.id == pair_id)
            .ok_or_else(|| "Pair not found.".to_string())?;

        let was_hidden = pair.hidden;
        if was_hidden {
            pair.hidden = false;
            settings::save(&app, &current);
        }

        (was_hidden, current.clone())
    };

    {
        let mut hidden_before_arrange = state.hidden_before_arrange.lock().unwrap();
        if was_hidden {
            hidden_before_arrange.insert(pair_id.clone(), true);
        } else {
            hidden_before_arrange.remove(&pair_id);
        }
    }

    let pair = settings
        .pairs
        .iter()
        .find(|pair| pair.id == pair_id)
        .cloned()
        .ok_or_else(|| "Pair not found.".to_string())?;

    ensure_overlay_window(&app, &pair)?;

    let window = app
        .get_webview_window(&overlay_label(&pair_id))
        .ok_or_else(|| "Overlay window not available.".to_string())?;

    {
        let mut arranging_pairs = state.arranging_pairs.lock().unwrap();
        arranging_pairs.insert(pair_id.clone());
    }

    window.show().map_err(|err| err.to_string())?;
    window.set_focus().ok();
    window
        .set_ignore_cursor_events(false)
        .map_err(|err| err.to_string())?;
    window
        .emit(
            "pair-state-changed",
            PairStatePayload {
                pair,
                arranging: true,
            },
        )
        .map_err(|err| err.to_string())?;

    Ok(())
}

#[tauri::command]
fn finish_arranging_pair(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    pair_id: String,
) -> Result<Settings, String> {
    let label = overlay_label(&pair_id);
    let restore_hidden = state
        .hidden_before_arrange
        .lock()
        .unwrap()
        .get(&pair_id)
        .copied()
        .unwrap_or(false);
    let window = app
        .get_webview_window(&label)
        .ok_or_else(|| "Overlay window not available.".to_string())?;
    let position = window.outer_position().map_err(|err| err.to_string())?;

    {
        let mut current = state.settings.lock().unwrap();
        let pair = current
            .pairs
            .iter_mut()
            .find(|pair| pair.id == pair_id)
            .ok_or_else(|| "Pair not found.".to_string())?;
        pair.placement_mode = "custom".to_string();
        pair.x = Some(position.x);
        pair.y = Some(position.y);
        if restore_hidden {
            pair.hidden = true;
        }
        settings::save(&app, &current);
    }

    state.arranging_pairs.lock().unwrap().remove(&pair_id);
    state.hidden_before_arrange.lock().unwrap().remove(&pair_id);
    window
        .set_ignore_cursor_events(true)
        .map_err(|err| err.to_string())?;

    let cloned = state.settings.lock().unwrap().clone();
    sync_overlay_windows(&app, &cloned);

    Ok(cloned)
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PairStatePayload {
    pair: EyePairSettings,
    arranging: bool,
}

fn overlay_label(pair_id: &str) -> String {
    format!("overlay-{pair_id}")
}

fn get_pair_position(app: &tauri::AppHandle, pair: &EyePairSettings) -> (f64, f64) {
    let monitor = app
        .primary_monitor()
        .ok()
        .flatten()
        .expect("no primary monitor");
    let size = monitor.size();
    let scale = monitor.scale_factor();
    let w = size.width as f64 / scale;
    let h = size.height as f64 / scale;

    let (top_margin, bottom_margin) = if cfg!(target_os = "macos") {
        (45.0, 90.0)
    } else {
        (DEFAULT_MARGIN, 68.0)
    };

    match pair.corner.as_str() {
        "top-left" => (DEFAULT_MARGIN, top_margin),
        "top-right" => (w - OVERLAY_WIDTH - DEFAULT_MARGIN, top_margin),
        "bottom-left" => (DEFAULT_MARGIN, h - OVERLAY_HEIGHT - bottom_margin),
        "centre" => ((w - OVERLAY_WIDTH) / 2.0, (h - OVERLAY_HEIGHT) / 2.0),
        _ => (
            w - OVERLAY_WIDTH - DEFAULT_MARGIN,
            h - OVERLAY_HEIGHT - bottom_margin,
        ),
    }
}

fn set_window_position(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    pair: &EyePairSettings,
) {
    if pair.placement_mode == "custom" {
        if let (Some(x), Some(y)) = (pair.x, pair.y) {
            window
                .set_position(Position::Physical(PhysicalPosition { x, y }))
                .ok();
            return;
        }
    }

    let (x, y) = get_pair_position(app, pair);
    window
        .set_position(Position::Logical(tauri::LogicalPosition { x, y }))
        .ok();
}

fn ensure_overlay_window(app: &tauri::AppHandle, pair: &EyePairSettings) -> Result<(), String> {
    let label = overlay_label(&pair.id);
    if app.get_webview_window(&label).is_some() {
        return Ok(());
    }

    let mut builder = WebviewWindowBuilder::new(
        app,
        &label,
        WebviewUrl::App(format!("overlay.html?pairId={}", pair.id).into()),
    )
    .title(&pair.name)
    .inner_size(OVERLAY_WIDTH, OVERLAY_HEIGHT)
    .position(0.0, 0.0)
    .decorations(false)
    .background_color(tauri::webview::Color(0, 0, 0, 0))
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .resizable(false)
    .visible(false);

    #[cfg(target_os = "windows")]
    {
        builder = builder.transparent(true).shadow(false);
    }
    #[cfg(target_os = "macos")]
    {
        builder = builder.title_bar_style(tauri::TitleBarStyle::Transparent);
    }

    let overlay = builder.build().map_err(|err| err.to_string())?;
    overlay
        .set_ignore_cursor_events(true)
        .map_err(|err| err.to_string())?;
    set_window_position(app, &overlay, pair);

    Ok(())
}

fn sync_overlay_windows(app: &tauri::AppHandle, settings: &Settings) {
    let expected_labels: HashSet<String> = settings
        .pairs
        .iter()
        .map(|pair| overlay_label(&pair.id))
        .collect();

    for (label, window) in app.webview_windows() {
        if label.starts_with("overlay-") && !expected_labels.contains(label.as_str()) {
            window.close().ok();
        }
    }

    let arranging_pairs = app
        .state::<AppState>()
        .arranging_pairs
        .lock()
        .unwrap()
        .clone();

    for pair in &settings.pairs {
        if ensure_overlay_window(app, pair).is_err() {
            continue;
        }

        if let Some(window) = app.get_webview_window(&overlay_label(&pair.id)) {
            set_window_position(app, &window, pair);
            window.set_title(&pair.name).ok();

            let arranging = arranging_pairs.contains(&pair.id);
            window.set_ignore_cursor_events(!arranging).ok();
            if pair.hidden && !arranging {
                window.hide().ok();
            }
            window
                .emit(
                    "pair-state-changed",
                    PairStatePayload {
                        pair: pair.clone(),
                        arranging,
                    },
                )
                .ok();
            if arranging {
                window.show().ok();
            }
        }
    }
}

fn start_blink_timer(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut next_blinks: HashMap<String, Instant> = HashMap::new();
        let mut hide_at: HashMap<String, Instant> = HashMap::new();

        loop {
            let state = app.state::<AppState>();
            let settings = state.settings.lock().unwrap().clone();
            let arranging_pairs = state.arranging_pairs.lock().unwrap().clone();
            drop(state);

            let now = Instant::now();
            let active_ids: HashSet<String> =
                settings.pairs.iter().map(|pair| pair.id.clone()).collect();
            next_blinks.retain(|pair_id, _| active_ids.contains(pair_id));
            hide_at.retain(|pair_id, _| active_ids.contains(pair_id));

            for pair in &settings.pairs {
                if pair.hidden {
                    hide_at.remove(&pair.id);
                    continue;
                }

                let next_blink = next_blinks
                    .entry(pair.id.clone())
                    .or_insert_with(|| now + Duration::from_millis(pair.interval));

                if arranging_pairs.contains(&pair.id) || now < *next_blink {
                    continue;
                }

                let label = overlay_label(&pair.id);
                if let Some(window) = app.get_webview_window(&label) {
                    window.show().ok();
                    hide_at.insert(
                        pair.id.clone(),
                        now + Duration::from_millis(pair.display_duration),
                    );
                }

                let delay = pair.interval.max(pair.display_duration);
                *next_blink = now + Duration::from_millis(delay);
            }

            let due_to_hide: Vec<String> = hide_at
                .iter()
                .filter(|(_, hide_time)| now >= **hide_time)
                .map(|(pair_id, _)| pair_id.clone())
                .collect();

            for pair_id in due_to_hide {
                hide_at.remove(&pair_id);
                if arranging_pairs.contains(&pair_id) {
                    continue;
                }
                if let Some(window) = app.get_webview_window(&overlay_label(&pair_id)) {
                    window.hide().ok();
                }
            }

            std::thread::sleep(Duration::from_millis(200));
        }
    });
}

fn setup_tray(app: &tauri::AppHandle) {
    let settings_item = MenuItemBuilder::with_id("settings", "Settings")
        .build(app)
        .unwrap();
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app).unwrap();
    let menu = MenuBuilder::new(app)
        .item(&settings_item)
        .separator()
        .item(&quit_item)
        .build()
        .unwrap();

    let icon = Image::from_bytes(include_bytes!("../../src/assets/icon.png"))
        .expect("failed to load tray icon");

    TrayIconBuilder::new()
        .icon(icon)
        .tooltip("The Blinking Guy")
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "settings" => {
                if let Some(window) = app.get_webview_window("settings") {
                    window.show().ok();
                    window.set_focus().ok();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("settings") {
                    window.show().ok();
                    window.set_focus().ok();
                }
            }
        })
        .build(app)
        .unwrap();
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(AppState {
            settings: Mutex::new(Settings::default()),
            arranging_pairs: Mutex::new(HashSet::new()),
            hidden_before_arrange: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            create_eye_pair,
            delete_eye_pair,
            start_arranging_pair,
            finish_arranging_pair
        ])
        .setup(|app| {
            let loaded = settings::load(&app.handle());
            {
                let state = app.state::<AppState>();
                *state.settings.lock().unwrap() = loaded.clone();
            }

            setup_tray(app.handle());
            if let Err(err) = apply_start_on_boot(app.handle(), loaded.start_on_boot) {
                eprintln!("failed to sync autostart state: {err}");
            }
            sync_overlay_windows(app.handle(), &loaded);
            start_blink_timer(app.handle().clone());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
