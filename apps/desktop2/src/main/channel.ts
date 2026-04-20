import { app } from "electron";
import { readFileSync } from "node:fs";
import path from "node:path";

export type Channel = "staging" | "nightly" | "stable";

// Channel → product identity. Mirrors the 1:1 mapping encoded in
// `apps/desktop/src-tauri/tauri.conf.<channel>.json` (just re-namespaced
// under `com.char.*` for the Electron port). Each value is a distinct
// install on disk, a distinct `userData` dir, and a distinct auto-update
// feed slot.
export const CHANNEL_APP_IDS: Record<Channel, string> = {
  stable: "com.char.stable",
  nightly: "com.char.nightly",
  staging: "com.char.staging",
};

export const CHANNEL_PRODUCT_NAMES: Record<Channel, string> = {
  stable: "Char",
  nightly: "Char Nightly",
  staging: "Char Staging",
};

// Resolve order:
//   1. Packaged `package.json#hyprChannel` — stamped by
//      `electron-builder.config.ts` via `extraMetadata` at bundle time.
//   2. `HYPR_CHANNEL` env var — used in `pnpm dev`.
//   3. `"staging"` — safe default; never claims a production `appId`.
function resolveChannel(): Channel {
  try {
    const pkgPath = path.join(app.getAppPath(), "package.json");
    const raw = JSON.parse(readFileSync(pkgPath, "utf8"))?.hyprChannel;
    if (raw === "staging" || raw === "nightly" || raw === "stable") return raw;
  } catch {
    // Missing / unreadable package.json (e.g. during tests) → fall through.
  }

  const env = process.env.HYPR_CHANNEL;
  if (env === "staging" || env === "nightly" || env === "stable") return env;

  return "staging";
}

export const CHANNEL: Channel = resolveChannel();
export const APP_ID: string = CHANNEL_APP_IDS[CHANNEL];
export const PRODUCT_NAME: string = CHANNEL_PRODUCT_NAMES[CHANNEL];

// Off by default on staging so dev builds never phone home. Mirrors both
// `tauri.conf.staging.json` (`updater.active: false`) and OpenCode's
// `UPDATER_ENABLED = app.isPackaged && CHANNEL !== "dev"`. The auto-updater
// itself is not wired yet; this is the flag future wiring should read.
export const UPDATER_ENABLED: boolean = app.isPackaged && CHANNEL !== "staging";
