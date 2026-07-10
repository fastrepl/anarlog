import { createDb } from "@hypr/db";
import { createUseDrizzleLiveQuery, createUseLiveQuery } from "@hypr/db-react";
import { tauriLiveQueryClient } from "@hypr/db-tauri";

export const liveQueryClient = tauriLiveQueryClient;
export const db = createDb(liveQueryClient);
export const useLiveQuery = createUseLiveQuery(liveQueryClient);
export const useDrizzleLiveQuery = createUseDrizzleLiveQuery(liveQueryClient);
