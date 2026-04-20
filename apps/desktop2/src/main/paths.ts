import { app } from "electron";
import path from "node:path";

import { APP_ID } from "./channel.js";

// Channel-scoped path so stable / nightly / staging never share a db.
// Once the NAPI side accepts a path again, wire this in through `sdk.init`.
export function dbPath(): string {
  return path.join(app.getPath("appData"), APP_ID, "app.db");
}

// Resolve a file shipped as `extraResources` in `electron-builder.config.ts`.
// In dev we read straight from the repo; packaged builds read from
// `process.resourcesPath` (Electron's convention for `extraResources`).
export function resourcePath(relative: string): string {
  if (app.isPackaged) {
    return path.join(process.resourcesPath, relative);
  }

  // `app.getAppPath()` in dev points at `apps/desktop2`; walk up to the
  // monorepo root so we can reference assets that live alongside Tauri.
  const workspaceRoot = path.resolve(app.getAppPath(), "..", "..");
  return path.join(workspaceRoot, relative);
}

// Resolve the packaged `char` CLI. Unlike `resourcePath`, this lives under
// `<App>.app/Contents/MacOS/` (see `electron-builder.config.ts#mac.extraFiles`)
// so electron-builder co-signs it with the main identity — mirroring Tauri's
// sidecar layout in `apps/desktop/src-tauri/src/embedded_cli.rs`.
//
// Returns `null` on non-macOS — embedded CLI is macOS-only for now, matching
// the Tauri side. In dev the binary lives under `apps/desktop2/binaries/`
// produced by `cargo xtask prepare-desktop2-binaries`.
export function embeddedCliPath(): string | null {
  if (process.platform !== "darwin") return null;

  if (app.isPackaged) {
    return path.join(path.dirname(app.getPath("exe")), "char-cli");
  }

  const triple = (() => {
    if (process.arch === "arm64") return "aarch64-apple-darwin";
    if (process.arch === "x64") return "x86_64-apple-darwin";
    return null;
  })();
  if (!triple) return null;

  const workspaceRoot = path.resolve(app.getAppPath(), "..", "..");
  return path.join(
    workspaceRoot,
    "apps/desktop2/binaries",
    `char-cli-${triple}`,
  );
}
