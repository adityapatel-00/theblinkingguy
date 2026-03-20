#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod settings;

use settings::Settings;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};

struct AppState {
    settings: Mutex<Settings>,
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    new_settings: Settings,
) -> Settings {
    let mut current = state.settings.lock().unwrap();
    *current = new_settings;
    settings::save(&app, &current);

    // Notify overlay of style change
    if let Some(overlay) = app.get_webview_window("overlay") {
        overlay.emit("style-changed", &current.style).ok();
    }

    // Reposition overlay
    reposition_overlay(&app, &current);

    current.clone()
}

fn get_overlay_position(app: &tauri::AppHandle, corner: &str) -> (f64, f64) {
    let monitor = app
        .primary_monitor()
        .ok()
        .flatten()
        .expect("no primary monitor");
    let size = monitor.size();
    let scale = monitor.scale_factor();
    let w = size.width as f64 / scale;
    let h = size.height as f64 / scale;
    let ow = 140.0;
    let oh = 80.0;
    let margin = 20.0;

    // OS-aware margins: monitor.size() returns full screen (not work area),
    // so we compensate for taskbar/dock depending on platform.
    // Windows: taskbar at bottom (~48px), macOS: dock at bottom (~70px) + menu bar at top (~25px)
    let (top_margin, bottom_margin) = if cfg!(target_os = "macos") {
        (45.0, 90.0) // menu bar + some padding, dock + padding
    } else {
        (margin, 68.0) // standard Windows taskbar
    };

    match corner {
        "top-left" => (margin, top_margin),
        "top-right" => (w - ow - margin, top_margin),
        "bottom-left" => (margin, h - oh - bottom_margin),
        "centre" => ((w - ow) / 2.0, (h - oh) / 2.0),
        _ => (w - ow - margin, h - oh - bottom_margin), // bottom-right default
    }
}

fn reposition_overlay(app: &tauri::AppHandle, settings: &Settings) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        let (x, y) = get_overlay_position(app, &settings.corner);
        overlay
            .set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }))
            .ok();
    }
}

fn create_overlay(app: &tauri::AppHandle, settings: &Settings) {
    let (x, y) = get_overlay_position(app, &settings.corner);

    let overlay = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("overlay.html".into()))
        .title("Overlay")
        .inner_size(140.0, 80.0)
        .position(x, y)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .background_color(tauri::webview::Color(0, 0, 0, 0))
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .resizable(false)
        .visible(false)
        .build()
        .expect("failed to create overlay window");

    overlay.set_ignore_cursor_events(true).ok();
}

fn start_blink_timer(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        let state = app.state::<AppState>();
        let settings = state.settings.lock().unwrap().clone();
        drop(state);

        std::thread::sleep(Duration::from_millis(settings.interval));

        if let Some(overlay) = app.get_webview_window("overlay") {
            overlay.show().ok();
            std::thread::sleep(Duration::from_millis(settings.display_duration));
            overlay.hide().ok();
        }
    });
}

fn setup_tray(app: &tauri::AppHandle) {
    let settings_item = MenuItemBuilder::with_id("settings", "Settings")
        .build(app)
        .unwrap();
    let quit_item = MenuItemBuilder::with_id("quit", "Quit")
        .build(app)
        .unwrap();
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
                if let Some(w) = app.get_webview_window("settings") {
                    w.show().ok();
                    w.set_focus().ok();
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
                if let Some(w) = app.get_webview_window("settings") {
                    w.show().ok();
                    w.set_focus().ok();
                }
            }
        })
        .build(app)
        .unwrap();
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            settings: Mutex::new(Settings::default()),
        })
        .invoke_handler(tauri::generate_handler![get_settings, save_settings])
        .setup(|app| {
            // Load persisted settings into state
            let loaded = settings::load(&app.handle());
            {
                let state = app.state::<AppState>();
                *state.settings.lock().unwrap() = loaded.clone();
            }

            setup_tray(app.handle());
            create_overlay(app.handle(), &loaded);
            start_blink_timer(app.handle().clone());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
