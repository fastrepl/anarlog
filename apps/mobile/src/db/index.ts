import { createUseLiveQuery } from "@anlg/db-react";

import { mobileLiveQueryClient, mobileTransactionClient } from "@/db/client";

export {
  bootstrapE2eeReplica,
  configureE2eeReplica,
  generateE2eeRecoveryKey,
  getSyncStatus,
  inspectE2eeRecoveryKey,
  startSync,
  stopSync,
  syncNow,
  type E2eeRecoveryKeyIdentity,
  type MobileSyncStatus,
} from "@/db/client";

export const liveQueryClient = mobileLiveQueryClient;
export const useLiveQuery = createUseLiveQuery(liveQueryClient);
export const executeTransaction = mobileTransactionClient.executeTransaction;
export const execute = liveQueryClient.execute;
