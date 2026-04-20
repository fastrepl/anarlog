// IPC channel names shared between Electron main and preload. Both sides
// import from this module so we never inline `"hypr:…"` string literals.
// Channel namespacing mirrors the Tauri `plugin:db|…` command naming so the
// two transports stay recognizable side by side.
//
// Updater channels live next to their event/snapshot types in `./updater.ts`
// under `updaterIpcChannels`; don't mirror them here.
export const hyprIpcChannels = {
  dbExecute: "hypr:db:execute",
  dbExecuteProxy: "hypr:db:executeProxy",
  dbSubscribe: "hypr:db:subscribe",
  dbUnsubscribe: "hypr:db:unsubscribe",
  openExternal: "hypr:openExternal",
  embeddedCliCheck: "hypr:embeddedCli:check",
  embeddedCliInstall: "hypr:embeddedCli:install",
  embeddedCliUninstall: "hypr:embeddedCli:uninstall",
} as const;

export type HyprIpcChannel =
  (typeof hyprIpcChannels)[keyof typeof hyprIpcChannels];
