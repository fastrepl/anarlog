import type { LiveQueryClient } from "@hypr/db-runtime";
import { execute, subscribe } from "@hypr/plugin-db";

export const tauriLiveQueryClient: LiveQueryClient = {
  execute,
  subscribe,
};
