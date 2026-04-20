import { createDb } from "@hypr/db";
import { electronLiveQueryClient } from "@hypr/db-electron";
import { createUseDrizzleLiveQuery, createUseLiveQuery } from "@hypr/db-react";

// Renderer-side data-access singleton. Mirrors `apps/desktop/src/db/index.ts`
// on the Tauri side — same drizzle handle, same hooks, different transport.
// Hook usage: `useDrizzleLiveQuery(db.select().from(sessions)...)`.
export const db = createDb(electronLiveQueryClient);
export const useLiveQuery = createUseLiveQuery(electronLiveQueryClient);
export const useDrizzleLiveQuery = createUseDrizzleLiveQuery(
  electronLiveQueryClient,
);
