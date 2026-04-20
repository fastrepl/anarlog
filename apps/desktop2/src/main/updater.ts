import { BrowserWindow, ipcMain } from "electron";
import electronUpdater from "electron-updater";

import type { UpdaterEvent } from "../shared/updater.js";
import { updaterIpcChannels } from "../shared/updater.js";
import { UPDATER_ENABLED } from "./channel.js";

// electron-updater is published as CommonJS, so we have to read `autoUpdater`
// off the default export when consuming it from an ESM context. Mirrors the
// workaround documented at https://www.electron.build/auto-update.
const { autoUpdater } = electronUpdater;

let pendingInstallVersion: string | null = null;
let wired = false;

function broadcast(event: UpdaterEvent) {
  for (const window of BrowserWindow.getAllWindows()) {
    if (window.isDestroyed()) {
      continue;
    }
    window.webContents.send(updaterIpcChannels.event, event);
  }
}

// `autoDownload: true` is the electron-updater default; we still pin it so
// the banner's "downloading" state is always exercised via real events.
// `autoInstallOnAppQuit: false` keeps installation explicit (the renderer
// has to call `updater.install()`), matching the main2 flow where the user
// pressed "Update & Restart".
function configureAutoUpdater() {
  autoUpdater.autoDownload = true;
  autoUpdater.autoInstallOnAppQuit = false;

  autoUpdater.on("checking-for-update", () => {
    broadcast({ type: "checking" });
  });

  autoUpdater.on("update-available", (info) => {
    // Cache the pending version as soon as we know one is on the way so the
    // download-progress events carry it forward (electron-updater doesn't
    // re-emit the version on progress ticks).
    pendingInstallVersion = info.version;
    broadcast({ type: "available", version: info.version });
  });

  autoUpdater.on("update-not-available", () => {
    broadcast({ type: "not-available" });
  });

  autoUpdater.on("download-progress", (progress) => {
    broadcast({
      type: "download-progress",
      version: pendingInstallVersion ?? "",
      transferred: progress.transferred,
      total: progress.total,
      percent: progress.percent,
    });
  });

  autoUpdater.on("update-downloaded", (info) => {
    pendingInstallVersion = info.version;
    broadcast({ type: "ready", version: info.version });
  });

  autoUpdater.on("error", (error) => {
    broadcast({ type: "error", message: error.message });
  });
}

export function registerUpdaterHandlers() {
  if (wired) {
    return;
  }
  wired = true;

  ipcMain.handle(updaterIpcChannels.check, async () => {
    if (!UPDATER_ENABLED) {
      broadcast({ type: "not-available" });
      return;
    }

    try {
      await autoUpdater.checkForUpdates();
    } catch (error) {
      broadcast({
        type: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  });

  ipcMain.handle(updaterIpcChannels.install, async () => {
    if (!pendingInstallVersion) {
      return;
    }

    // `quitAndInstall(isSilent, isForceRunAfter)` — defaults match the
    // "quit, install, relaunch" behaviour the main2 banner promised with
    // its "Update & Restart" button.
    autoUpdater.quitAndInstall(false, true);
  });

  if (UPDATER_ENABLED) {
    configureAutoUpdater();
  }
}

// Initial check kicks off shortly after the main window appears so the first
// release fetch doesn't block startup. Only invoked when the channel is a
// real release channel (staging/dev builds skip).
export function scheduleInitialUpdateCheck() {
  if (!UPDATER_ENABLED) {
    return;
  }

  setTimeout(() => {
    autoUpdater.checkForUpdates().catch((error: unknown) => {
      broadcast({
        type: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    });
  }, 5_000);
}
