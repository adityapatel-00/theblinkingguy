# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

"The Blinking Guy" is a Tauri v2 desktop app that reminds developers to blink by showing an animated eye overlay at a configurable screen corner on a timer. It runs as a system tray application on Windows.

## Commands

- **Run the app (dev):** `npm run tauri:dev` (compiles Rust + launches app with hot-reload)
- **Build for production:** `npm run tauri:build` (creates installer in `src-tauri/target/release/bundle/`)
- **Install dependencies:** `npm install` (JS), `cd src-tauri && cargo check` (Rust)

There is no test suite or linter configured.

## Architecture

Tauri v2 app with Rust backend + vanilla HTML/CSS/JS frontend:

### Backend (Rust) — `src-tauri/src/`

- **main.rs** — App entry point. Sets up system tray, creates overlay window at runtime, runs blink timer on a background thread, registers IPC command handlers (`get_settings`, `save_settings`). Manages `AppState` with a `Mutex<Settings>`.
- **settings.rs** — `Settings` struct with serde serialization (`camelCase` for JS compatibility). Load/save to JSON in the app data directory.

### Frontend — `src/`

- **overlay.html** — Frameless, transparent, always-on-top window (140×80px) that displays animated blinking eyes. Contains 47 eye styles across 7 categories, all as CSS classes. JS-driven synchronized blink animation. Thor style has random SVG lightning bolts.
- **settings.html** — Settings UI with tabbed/scrollable eye style picker (categories: Modern, Fantasy, Animals, Cultural, Historic, Spooky, Glam), interval/duration dropdowns, 5-position grid, and live position preview. Communicates with Rust via `window.__TAURI__.core.invoke()`.

### Configuration — `src-tauri/`

- **tauri.conf.json** — Window definitions (settings window static, overlay created at runtime), tray icon config, build settings.
- **capabilities/default.json** — IPC permissions for both windows.
- **Cargo.toml** — Rust dependencies: tauri (tray-icon, image-png features), serde, serde_json.

### Key patterns

- The overlay window is created once at startup and shown/hidden on each blink cycle — it is never recreated.
- `set_ignore_cursor_events(true)` makes the overlay click-through.
- Settings changes trigger overlay repositioning and notify the overlay of style changes via the `style-changed` Tauri event.
- The blink timer runs on a dedicated Rust thread, re-reading settings from shared state each iteration.
- All UI (styles, animations, layout) is inline within the HTML files — there are no external CSS or JS files.
- Frontend uses `withGlobalTauri: true` to access Tauri APIs via `window.__TAURI__` without npm imports.
