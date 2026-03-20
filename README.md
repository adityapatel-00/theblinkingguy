# The Blinking Guy 👀

A lightweight desktop app that reminds you to blink. Because developers forget.

It sits in your system tray and periodically flashes a pair of animated eyes on your screen — a gentle nudge to give your eyes a break.

Built with [Tauri v2](https://v2.tauri.app/) for a fast, tiny footprint (~5MB).

## Features

- **47 eye styles** across 7 categories — Modern, Fantasy, Animals, Cultural, Historic, Spooky, Glam
- **Configurable position** — pick any corner or centre of your screen
- **Adjustable timing** — set the reminder interval (2s–60s) and display duration (0.5s–10s)
- **Click-through overlay** — the eyes never steal focus or block your work
- **System tray app** — runs quietly in the background
- **Transparent overlay** — eyes float seamlessly on your screen with no borders

## Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/) (latest stable)
- Microsoft C++ Build Tools (via Visual Studio Installer)
- WebView2 (pre-installed on Windows 10/11)

## Getting Started

```bash
npm install
npm run tauri:dev
```

First build compiles ~400 Rust crates and takes a few minutes. Subsequent builds are fast (~5–10s).

## Build for Production

```bash
npm run tauri:build
```

Creates an installer in `src-tauri/target/release/bundle/`.

## Usage

- **Right-click** the tray icon → **Settings** to configure
- **Left-click** the tray icon to open settings
- Browse eye styles by category (Modern, Fantasy, Animals, Cultural, Historic, Spooky, Glam)
- Choose your preferred screen corner, interval, and display duration

## Tech Stack

- **Backend:** Rust + Tauri v2
- **Frontend:** Vanilla HTML/CSS/JS (no framework)
- **Window management:** Transparent, always-on-top overlay with click-through

## License

[MIT](LICENSE)
