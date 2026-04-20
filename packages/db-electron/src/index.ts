import type { DrizzleProxyClient, LiveQueryClient } from "@hypr/db-runtime";

// Mirror of `@hypr/db-tauri`. Binds drizzle's proxy driver + live-query
// hooks to the Electron preload surface (`window.hypr.db`). The full
// preload contract lives in `apps/desktop2/shared/src/api.ts`; we type
// only the slice this client needs so `window.hypr` stays declared by
// the consuming app.
type HyprGlobalDb = DrizzleProxyClient & LiveQueryClient;

function getDb(): HyprGlobalDb {
  const global = window as unknown as { hypr?: { db?: HyprGlobalDb } };
  const db = global.hypr?.db;
  if (!db) {
    throw new Error(
      "@hypr/db-electron: window.hypr.db is not available (preload not ready).",
    );
  }
  return db;
}

export const electronLiveQueryClient: LiveQueryClient & DrizzleProxyClient = {
  execute: (sql, params) => getDb().execute(sql, params),
  executeProxy: (sql, params, method) =>
    getDb().executeProxy(sql, params, method),
  subscribe: (sql, params, options) => getDb().subscribe(sql, params, options),
};
