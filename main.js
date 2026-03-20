const { app, BrowserWindow, Tray, Menu, screen, ipcMain, nativeImage } = require('electron');
const path = require('path');
const fs = require('fs');

let tray = null;
let overlayWindow = null;
let settingsWindow = null;
let blinkTimer = null;
let SETTINGS_PATH = null;

const DEFAULT_SETTINGS = {
  interval: 5000,
  corner: 'bottom-right',
  displayDuration: 1000,
  style: 'classic',
};

function loadSettings() {
  try {
    return { ...DEFAULT_SETTINGS, ...JSON.parse(fs.readFileSync(SETTINGS_PATH, 'utf-8')) };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

function saveSettings(settings) {
  fs.writeFileSync(SETTINGS_PATH, JSON.stringify(settings, null, 2));
}

function getOverlayPosition(corner) {
  const display = screen.getPrimaryDisplay();
  const { width, height } = display.workAreaSize;
  const w = 140;
  const h = 80;
  const margin = 20;

  const positions = {
    'top-left': { x: margin, y: margin },
    'top-right': { x: width - w - margin, y: margin },
    'bottom-left': { x: margin, y: height - h - margin },
    'bottom-right': { x: width - w - margin, y: height - h - margin },
    'centre': { x: Math.round((width - w) / 2), y: Math.round((height - h) / 2) },
  };

  return positions[corner] || positions['bottom-right'];
}

function createOverlayWindow() {
  const settings = loadSettings();
  const pos = getOverlayPosition(settings.corner);

  overlayWindow = new BrowserWindow({
    width: 140,
    height: 80,
    x: pos.x,
    y: pos.y,
    frame: false,
    transparent: true,
    alwaysOnTop: true,
    skipTaskbar: true,
    focusable: false,
    resizable: false,
    show: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
    },
  });

  overlayWindow.setIgnoreMouseEvents(true);
  overlayWindow.setAlwaysOnTop(true, 'floating');
  overlayWindow.loadFile('overlay.html');
}

function startBlinking() {
  stopBlinking();

  const settings = loadSettings();

  blinkTimer = setInterval(() => {
    if (overlayWindow && !overlayWindow.isDestroyed()) {
      overlayWindow.show();
      setTimeout(() => {
        if (overlayWindow && !overlayWindow.isDestroyed()) {
          overlayWindow.hide();
        }
      }, settings.displayDuration);
    }
  }, settings.interval);
}

function stopBlinking() {
  if (blinkTimer) {
    clearInterval(blinkTimer);
    blinkTimer = null;
  }
}

function repositionOverlay() {
  if (!overlayWindow || overlayWindow.isDestroyed()) return;
  const settings = loadSettings();
  const pos = getOverlayPosition(settings.corner);
  overlayWindow.setPosition(pos.x, pos.y);
}

function openSettings() {
  if (settingsWindow && !settingsWindow.isDestroyed()) {
    settingsWindow.focus();
    return;
  }

  settingsWindow = new BrowserWindow({
    width: 420,
    height: 750,
    resizable: false,
    minimizable: false,
    maximizable: false,
    title: 'The Blinking Guy - Settings',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
    },
  });

  settingsWindow.setMenu(null);
  settingsWindow.loadFile('settings.html');

  settingsWindow.on('closed', () => {
    settingsWindow = null;
  });
}

function createTray() {
  const iconPath = path.join(__dirname, 'assets', 'icon.png');
  const trayIcon = nativeImage.createFromPath(iconPath).resize({ width: 16, height: 16 });

  tray = new Tray(trayIcon);
  tray.setToolTip('The Blinking Guy');

  const contextMenu = Menu.buildFromTemplate([
    { label: 'Settings', click: openSettings },
    { type: 'separator' },
    { label: 'Quit', click: () => app.quit() },
  ]);

  tray.setContextMenu(contextMenu);
  tray.on('double-click', openSettings);
}

// App lifecycle
app.whenReady().then(() => {
  SETTINGS_PATH = path.join(app.getPath('userData'), 'settings.json');

  // IPC Handlers
  ipcMain.handle('get-settings', () => loadSettings());

  ipcMain.handle('save-settings', (_event, newSettings) => {
    const settings = { ...loadSettings(), ...newSettings };
    saveSettings(settings);
    repositionOverlay();
    startBlinking();

    // Notify overlay of style change
    if (overlayWindow && !overlayWindow.isDestroyed()) {
      overlayWindow.webContents.send('style-changed', settings.style);
    }

    return settings;
  });

  createTray();
  createOverlayWindow();
  startBlinking();
});

app.on('window-all-closed', () => {
  // Do nothing - keep app running in tray when settings window closes
});

app.on('before-quit', () => {
  stopBlinking();
});
