import type { DrizzleProxyClient, LiveQueryClient } from "@hypr/db-runtime";

import type { EmbeddedCliStatus } from "./embedded-cli.js";
import type { UpdaterEvent } from "./updater.js";

// Typed surface exposed to the renderer via
// `contextBridge.exposeInMainWorld("hypr", api)` in `preload.cts`.
//
// Everything data-related is the same generic surface as `@hypr/plugin-db`
// on the Tauri side — the renderer talks drizzle, not per-entity IPC. Only
// truly Electron-specific things (openExternal, embedded CLI, auto-update)
// get their own slots here.
export interface HyprElectronApi {
  db: DrizzleProxyClient & LiveQueryClient;
  openExternal(url: string): Promise<void>;
  embeddedCli: {
    check(): Promise<EmbeddedCliStatus>;
    install(): Promise<EmbeddedCliStatus>;
    uninstall(): Promise<EmbeddedCliStatus>;
  };
  updater: {
    check(): Promise<void>;
    install(): Promise<void>;
    subscribe(callback: (event: UpdaterEvent) => void): () => void;
  };
}
