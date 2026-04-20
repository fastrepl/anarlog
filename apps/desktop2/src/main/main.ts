import {
  BrowserWindow,
  app,
  ipcMain,
  nativeTheme,
  shell as electronShell,
} from "electron";
import path from "node:path";

import * as sdk from "@hypr/napi-sdk";

import { hyprIpcChannels } from "../shared/channels.js";
import type { DbSubscribeResult } from "../shared/subscribe.js";
import { APP_ID, PRODUCT_NAME } from "./channel.js";

// Channel-scoped identity. Runs BEFORE `app.whenReady()` (which Electron
// requires for `setPath("userData", …)`) and BEFORE any BrowserWindow is
// created. Each channel gets a distinct `appId`, taskbar/notification id,
// and `userData` directory so stable / nightly / staging install side-by-side
// without corrupting each other's db or settings. Mirrors
// `apps/desktop/src-tauri/tauri.conf.<channel>.json::identifier`.
app.setName(PRODUCT_NAME);
app.setAppUserModelId(APP_ID);
app.setPath("userData", path.join(app.getPath("appData"), APP_ID));

// Force light NSAppearance globally, matching Tauri's
// `theme(Some(tauri::Theme::Light))` in `plugins/windows/src/window/v1.rs`. It
// must be applied before any `BrowserWindow` is created, otherwise macOS keeps
// the system-wide appearance (and the traffic lights render dark).
if (process.platform === "darwin") {
  nativeTheme.themeSource = "light";
}

import * as embeddedCli from "./embedded-cli.js";
import { LiveQuerySubscriptionManager } from "./subscription-manager.js";
import { createTray } from "./tray.js";
import {
  registerUpdaterHandlers,
  scheduleInitialUpdateCheck,
} from "./updater.js";
import { createMainWindow } from "./window.js";

const liveQuerySubscriptions = new LiveQuerySubscriptionManager();
let mainWindow: BrowserWindow | null = null;
let trayHandle: ReturnType<typeof createTray> = null;

function showMainWindow() {
  if (!mainWindow || mainWindow.isDestroyed()) {
    mainWindow = createMainWindow();
    return;
  }

  if (process.platform === "darwin") {
    // When the app entered accessory mode (see `window-all-closed` below),
    // flipping back to a regular activation policy restores the dock icon.
    void app.setActivationPolicy?.("regular");
  }

  if (mainWindow.isMinimized()) {
    mainWindow.restore();
  }

  mainWindow.show();
  mainWindow.focus();
}

// Generic data-access surface, mirroring `plugin:db|{execute,execute_proxy,
// subscribe,unsubscribe}` on the Tauri side. The renderer uses drizzle
// (via `@hypr/db`) against `executeProxy`; `subscribe` is the reactive
// watch primitive. No per-entity IPC lives here.
function registerDbHandlers() {
  ipcMain.handle(
    hyprIpcChannels.dbExecute,
    (_event, sql: string, params: unknown[]) =>
      sdk.execute(sql, params as never[]),
  );

  ipcMain.handle(
    hyprIpcChannels.dbExecuteProxy,
    (_event, sql: string, params: unknown[], method: string) =>
      sdk.executeProxy(sql, params as never[], method),
  );

  ipcMain.handle(
    hyprIpcChannels.dbSubscribe,
    (event, sql: string, params: unknown[]): DbSubscribeResult => {
      const { channel, reactive } = liveQuerySubscriptions.start(
        sql,
        params,
        event.sender,
      );
      return { channel, reactive };
    },
  );

  ipcMain.handle(
    hyprIpcChannels.dbUnsubscribe,
    (_event, channel: string): boolean => {
      return liveQuerySubscriptions.stopByChannel(channel);
    },
  );
}

function registerNativeHandlers() {
  ipcMain.handle(hyprIpcChannels.openExternal, async (_event, url: string) => {
    await electronShell.openExternal(url);
  });
}

function registerEmbeddedCliHandlers() {
  // Embedded `char` CLI lifecycle. macOS-only (the check() status reports
  // `unsupported` elsewhere). See `src/main/embedded-cli.ts` for the port
  // of `apps/desktop/src-tauri/src/embedded_cli.rs`.
  ipcMain.handle(hyprIpcChannels.embeddedCliCheck, () => embeddedCli.check());
  ipcMain.handle(hyprIpcChannels.embeddedCliInstall, () =>
    embeddedCli.install(),
  );
  ipcMain.handle(hyprIpcChannels.embeddedCliUninstall, () =>
    embeddedCli.uninstall(),
  );
}

function registerIpcHandlers() {
  registerDbHandlers();
  registerNativeHandlers();
  registerEmbeddedCliHandlers();
  registerUpdaterHandlers();
}

// Mirrors `tauri_plugin_single_instance`: a second launch re-focuses the
// existing window instead of spinning up a duplicate app.
const gotSingleInstanceLock = app.requestSingleInstanceLock();
if (!gotSingleInstanceLock) {
  app.quit();
} else {
  registerIpcHandlers();

  app.on("second-instance", () => {
    showMainWindow();
  });

  app.whenReady().then(async () => {
    await sdk.init();
    mainWindow = createMainWindow();
    trayHandle = createTray({ onOpen: showMainWindow });
    scheduleInitialUpdateCheck();

    app.on("activate", () => {
      // Dock-click / Reopen: mirrors Tauri's `RunEvent::Reopen` handler.
      showMainWindow();
    });
  });
}

// On macOS we keep the process alive so the tray stays reachable, mirroring
// Tauri's `ExitRequested` → `ActivationPolicy::Accessory` handler. Everywhere
// else we follow the standard Electron convention and quit.
app.on("window-all-closed", () => {
  if (process.platform === "darwin") {
    void app.setActivationPolicy?.("accessory");
    return;
  }

  app.quit();
});

app.on("before-quit", () => {
  liveQuerySubscriptions.destroy();
  trayHandle?.destroy();
  trayHandle = null;
});
