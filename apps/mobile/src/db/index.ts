import { createUseLiveQuery } from "@anlg/db-react";

import { mobileLiveQueryClient, mobileTransactionClient } from "@/db/client";

export {
  bootstrapE2eeReplica,
  configureE2eeReplica,
  generateE2eeDeviceEnrollmentKey,
  generateE2eeRecoveryKey,
  getSyncStatus,
  inspectE2eeDeviceEnrollmentKey,
  inspectE2eeRecoveryKey,
  openE2eeDeviceEnrollment,
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
