import { createDb } from "@anlg/db";
import { createUseDrizzleLiveQuery, createUseLiveQuery } from "@anlg/db-react";
import { tauriLiveQueryClient, tauriTransactionClient } from "@anlg/db-tauri";

export const liveQueryClient = tauriLiveQueryClient;
export const db = createDb(liveQueryClient);
export const useLiveQuery = createUseLiveQuery(liveQueryClient);
export const useDrizzleLiveQuery = createUseDrizzleLiveQuery(liveQueryClient);
export const executeTransaction = tauriTransactionClient.executeTransaction;
