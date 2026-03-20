# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

"The Blinking Guy" is an Electron desktop app that reminds developers to blink by showing an animated eye overlay at a configurable screen corner on a timer. It runs as a system tray application on Windows.

## Commands

- **Run the app:** `npm start` (runs `electron .`)
- **Install dependencies:** `npm install`

There is no build step, test suite, or linter configured.

## Architecture

This is a single-process Electron app with three source files:

- **main.js** — Main process. Creates the system tray, manages two windows (overlay + settings), handles IPC, and runs the blink timer via `setInterval`. Settings are persisted as JSON in Electron's `userData` directory.
- **preload.js** — Exposes `window.api` with `getSettings()`, `saveSettings()`, and `onStyleChange()` via `contextBridge`.
- **overlay.html** — Frameless, transparent, always-on-top window (140×80px) that displays animated blinking eyes. Contains multiple eye styles (classic, anime, pixel, minimal, etc.) toggled via CSS classes. Shown/hidden by the main process timer.
- **settings.html** — Settings UI with controls for interval, screen corner, display duration, and eye style selection. Communicates with main process through the preload API.

### Key patterns

- The overlay window is created once and shown/hidden on each blink cycle — it is never recreated.
- `setIgnoreMouseEvents(true)` makes the overlay click-through.
- Settings changes from the settings window trigger `repositionOverlay()` + `startBlinking()` restart, and notify the overlay of style changes via `style-changed` IPC event.
- All UI (styles, animations, layout) is inline within the HTML files — there are no external CSS or JS files.
