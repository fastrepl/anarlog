// Updater channel / event contract shared between the Electron main process
// and the renderer. Trimmed to what `electron-updater` gives us natively,
// mirroring the shape of `@hypr/plugin-updater2` events from the Tauri build.

export const updaterIpcChannels = {
  check: "hypr:updater:check",
  install: "hypr:updater:install",
  // Broadcast channel for push events from main → every renderer. We use
  // `webContents.send(eventChannel, …)` and renderers listen via
  // `ipcRenderer.on(eventChannel, …)`.
  event: "hypr:updater:event",
} as const;

export type UpdaterIpcChannel =
  (typeof updaterIpcChannels)[keyof typeof updaterIpcChannels];

export type UpdaterStatus =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "ready"
  | "not-available"
  | "error";

// Discriminated union of push events the main process sends to renderers.
// Kept flat so the IPC boundary stays JSON-safe.
export type UpdaterEvent =
  | { type: "checking" }
  | { type: "not-available" }
  | { type: "available"; version: string }
  | {
      type: "download-progress";
      version: string;
      transferred: number;
      total: number;
      percent: number;
    }
  | { type: "ready"; version: string }
  | { type: "error"; message: string };

// Renderer-visible snapshot, derived from the event stream. Containers hold
// this in state and render a banner when `version` is set and `status` is
// either `available`, `downloading`, or `ready`.
export type UpdaterSnapshot = {
  status: UpdaterStatus;
  version: string | null;
  // 0..1 while downloading, null when not applicable.
  progress: number | null;
  errorMessage: string | null;
};

export const INITIAL_UPDATER_SNAPSHOT: UpdaterSnapshot = {
  status: "idle",
  version: null,
  progress: null,
  errorMessage: null,
};

export function reduceUpdaterSnapshot(
  state: UpdaterSnapshot,
  event: UpdaterEvent,
): UpdaterSnapshot {
  switch (event.type) {
    case "checking":
      return { ...state, status: "checking", errorMessage: null };
    case "not-available":
      return { ...state, status: "not-available", progress: null };
    case "available":
      return {
        status: "available",
        version: event.version,
        progress: null,
        errorMessage: null,
      };
    case "download-progress":
      return {
        status: "downloading",
        version: event.version,
        progress: event.total > 0 ? event.transferred / event.total : null,
        errorMessage: null,
      };
    case "ready":
      return {
        status: "ready",
        version: event.version,
        progress: 1,
        errorMessage: null,
      };
    case "error":
      return { ...state, status: "error", errorMessage: event.message };
  }
}

export interface UpdaterApi {
  check(): Promise<void>;
  install(): Promise<void>;
  subscribe(listener: (event: UpdaterEvent) => void): () => void;
}
