import { BrowserWindow, Menu, MenuItem, app, shell } from "electron";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { CHANNEL } from "./channel.js";
import { isAllowedExternalUrl } from "./url-allowlist.js";

const currentDir = path.dirname(fileURLToPath(import.meta.url));
const DEV_URL = process.env.ELECTRON_RENDERER_URL;

type MainWindowOptions = {
  preloadPath?: string;
};

// Mirrors `plugins/windows/src/window/v1.rs::AppWindow::Main`.
// On macOS we let traffic-light controls show but hide the title and let
// content extend under the title bar. On Windows/Linux we drop decorations
// entirely (`frame: false`), matching `decorations(false)` in the Tauri side.
export function createMainWindow(opts: MainWindowOptions = {}): BrowserWindow {
  const preload = opts.preloadPath ?? path.join(currentDir, "preload.cjs");
  const isDev = !app.isPackaged;

  const window = new BrowserWindow({
    width: 910,
    height: 600,
    minWidth: 620,
    minHeight: 500,
    show: false,
    // Paint the native frame with the stone-50 canvas color the UI uses, so
    // resizes and the pre-mount flash don't show a white seam under the
    // rounded content panels.
    backgroundColor: "#fafaf9",
    ...platformChrome(isDev),
    webPreferences: {
      preload,
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });

  window.once("ready-to-show", () => window.show());

  // Mirrors `.disable_drag_drop_handler()` on the Tauri builder: prevent the
  // webview from navigating to a file when the user drops one into the window.
  window.webContents.on("will-navigate", (event, url) => {
    if (DEV_URL && url.startsWith(DEV_URL)) return;
    if (!DEV_URL && url.startsWith("file://")) return;
    event.preventDefault();
  });

  // External links open in the user's browser rather than a new Electron window.
  // Gated through the same allowlist as the `hypr:openExternal` IPC so a
  // compromised renderer can't reach `file://`, `javascript:`, or custom
  // protocol handlers via `window.open` / `target="_blank"` either.
  window.webContents.setWindowOpenHandler(({ url }) => {
    if (isAllowedExternalUrl(url)) {
      void shell.openExternal(url);
    }
    return { action: "deny" };
  });

  if (isDebugBuild(isDev)) {
    attachDebugContextMenu(window);
  }

  void loadContent(window, isDev);

  return window;
}

function platformChrome(
  isDev: boolean,
): Electron.BrowserWindowConstructorOptions {
  switch (process.platform) {
    case "darwin":
      // Light theme is applied globally via `nativeTheme.themeSource`, matching
      // Tauri's `theme(Some(tauri::Theme::Light))`.
      return {
        titleBarStyle: "hiddenInset",
        trafficLightPosition: { x: 12, y: macosTrafficLightY(isDev) },
      };
    case "win32":
    case "linux":
    default:
      return { frame: false };
  }
}

// Vertically center the traffic lights inside the 40px (`h-10`) top bar that
// `src/renderer/shell/shell.view.tsx` renders, matching the `apps/desktop/src/main2/shell.tsx`
// reference. The y value does NOT translate 1:1 from Tauri's
// `traffic_light_position(12, 18)` even though both apps use the same top-bar
// CSS: Tauri's `TitleBarStyle::Overlay` places web content flush with the
// window top (y=0), while Electron's `hiddenInset` anchors the traffic-light
// widget higher than where the h-10 flex row's size-7 button lands, so we
// need a larger y to meet the button's visual center.
//
// If the traffic lights still look high/low compared to the home icon after
// a macOS update, nudge these two numbers — they are the entire contract.
function macosTrafficLightY(isDev: boolean): number {
  if (process.platform !== "darwin") {
    return 10;
  }

  // Electron exposes the macOS product version (e.g. "26.0.0") via
  // `process.getSystemVersion`, which maps cleanly to Tauri's `tauri_plugin_os`.
  const version = process.getSystemVersion?.() ?? "";
  const major = Number.parseInt(version.split(".")[0] ?? "0", 10);
  if (!Number.isFinite(major)) {
    return 10;
  }

  // macOS 26 (Tahoe) dev builds ship a taller traffic-light row in WebKit —
  // same reason the Tauri plugin bumps 18 → 24 in its equivalent branch.
  return major >= 26 && isDev ? 16 : 10;
}

// Electron ships no default context menu. We enable a minimal one on dev
// (`pnpm dev`), staging, and nightly builds so developers and internal testers
// get copy/paste + Inspect Element, while stable production builds remain
// locked down like the Tauri app.
function isDebugBuild(isDev: boolean): boolean {
  return isDev || CHANNEL !== "stable";
}

function attachDebugContextMenu(window: BrowserWindow): void {
  window.webContents.on("context-menu", (_event, params) => {
    const menu = new Menu();
    const { editFlags } = params;

    if (editFlags.canCut) menu.append(new MenuItem({ role: "cut" }));
    if (editFlags.canCopy) menu.append(new MenuItem({ role: "copy" }));
    if (editFlags.canPaste) menu.append(new MenuItem({ role: "paste" }));
    if (editFlags.canSelectAll) {
      menu.append(new MenuItem({ role: "selectAll" }));
    }

    if (menu.items.length > 0) {
      menu.append(new MenuItem({ type: "separator" }));
    }

    menu.append(
      new MenuItem({
        label: "Inspect Element",
        click: () => window.webContents.inspectElement(params.x, params.y),
      }),
    );
    menu.append(
      new MenuItem({
        label: "Toggle DevTools",
        accelerator:
          process.platform === "darwin" ? "Alt+Command+I" : "Ctrl+Shift+I",
        click: () => window.webContents.toggleDevTools(),
      }),
    );

    menu.popup({ window });
  });
}

async function loadContent(
  window: BrowserWindow,
  isDev: boolean,
): Promise<void> {
  if (isDev && DEV_URL) {
    // Don't auto-open DevTools — it's jarring as a detached window on every
    // launch, and electron-vite's dev server still honors the default
    // Electron shortcut (⌥⌘I / Ctrl+Shift+I) when the developer wants it.
    await window.loadURL(DEV_URL);
    return;
  }

  await window.loadFile(path.join(currentDir, "..", "ui", "index.html"));
}
