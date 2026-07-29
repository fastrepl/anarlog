import type {
  DrizzleProxyClient,
  LiveQueryClient,
  TransactionClient,
} from "@anlg/db-runtime";
import {
  execute,
  executeProxy,
  executeTransaction,
  subscribe,
} from "@anlg/plugin-db";

export const tauriLiveQueryClient: LiveQueryClient & DrizzleProxyClient = {
  execute,
  executeProxy,
  subscribe,
};

export const tauriTransactionClient: TransactionClient = {
  executeTransaction,
};
