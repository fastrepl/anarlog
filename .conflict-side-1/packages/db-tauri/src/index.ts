import type {
  DrizzleProxyClient,
  LiveQueryClient,
  TransactionClient,
} from "@hypr/db-runtime";
import {
  execute,
  executeProxy,
  executeTransaction,
  subscribe,
} from "@hypr/plugin-db";

export const tauriLiveQueryClient: LiveQueryClient & DrizzleProxyClient = {
  execute,
  executeProxy,
  subscribe,
};

export const tauriTransactionClient: TransactionClient = {
  executeTransaction,
};
