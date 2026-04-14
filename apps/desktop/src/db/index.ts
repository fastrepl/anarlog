import { createDb } from "@hypr/db";
import { createUseDrizzleLiveQuery, createUseLiveQuery } from "@hypr/db-react";
import { tauriLiveQueryClient } from "@hypr/db-tauri";

export const db = createDb(tauriLiveQueryClient);
export const useLiveQuery = createUseLiveQuery(tauriLiveQueryClient);
export const useDrizzleLiveQuery =
  createUseDrizzleLiveQuery(tauriLiveQueryClient);
