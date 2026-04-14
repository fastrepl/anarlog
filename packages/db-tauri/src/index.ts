import type { LiveQueryClient } from "@hypr/db-runtime";
import type { DrizzleProxyClient } from "@hypr/db-runtime";
import { execute, executeProxy, subscribe } from "@hypr/plugin-db";

export const tauriLiveQueryClient: LiveQueryClient & DrizzleProxyClient = {
  execute,
  executeProxy,
  subscribe,
};
