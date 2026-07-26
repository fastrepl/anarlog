import { createUseLiveQuery } from "@hypr/db-react";

import { mobileLiveQueryClient, mobileTransactionClient } from "@/db/client";

export const liveQueryClient = mobileLiveQueryClient;
export const useLiveQuery = createUseLiveQuery(liveQueryClient);
export const executeTransaction = mobileTransactionClient.executeTransaction;
export const execute = liveQueryClient.execute;
