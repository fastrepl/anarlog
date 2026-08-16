import { createUseLiveQuery } from "@anlg/db-react";

import { mobileLiveQueryClient, mobileTransactionClient } from "@/db/client";

export {
  configureE2eeReplica,
  getSyncStatus,
  startSync,
  stopSync,
  syncNow,
  type MobileSyncStatus,
} from "@/db/client";

export const liveQueryClient = mobileLiveQueryClient;
export const useLiveQuery = createUseLiveQuery(liveQueryClient);
export const executeTransaction = mobileTransactionClient.executeTransaction;
export const execute = liveQueryClient.execute;
