import { Directory, File, Paths } from "expo-file-system";

import type {
  LiveQueryClient,
  Row,
  TransactionClient,
  TransactionStatement,
  Unsubscribe,
} from "@anlg/db-runtime";
import {
  MobileDbBridge,
  type MobileDbBridgeLike,
  type QueryEventListener,
} from "@anlg/mobile-bridge";

import { captureOperationalError } from "@/lib/error-reporting";

let bridge: MobileDbBridgeLike | null = null;

function getBridge(): MobileDbBridgeLike {
  if (!bridge) {
    const databaseDirectory = new Directory(Paths.document, "SQLite");
    databaseDirectory.create({ intermediates: true, idempotent: true });
    const databaseUri = new File(databaseDirectory, "anarlog.db").uri;
    const databasePath = decodeURIComponent(
      databaseUri.replace(/^file:\/\//, ""),
    );
    bridge = MobileDbBridge.open(databasePath, "disabled");
  }
  return bridge;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function execute<T = Row>(
  sql: string,
  params: unknown[] = [],
): Promise<T[]> {
  try {
    return JSON.parse(getBridge().execute(sql, JSON.stringify(params))) as T[];
  } catch (error) {
    captureOperationalError(error, { operation: "database_execute" });
    throw error;
  }
}

async function executeTransaction(
  statements: TransactionStatement[],
): Promise<number[]> {
  try {
    return JSON.parse(
      getBridge().executeTransaction(JSON.stringify(statements)),
    ) as number[];
  } catch (error) {
    captureOperationalError(error, { operation: "database_transaction" });
    throw error;
  }
}

async function subscribe<T = Row>(
  sql: string,
  params: unknown[],
  options: { onData: (rows: T[]) => void; onError?: (error: string) => void },
): Promise<Unsubscribe> {
  let active = true;
  const listener: QueryEventListener = {
    onResult(rowsJson) {
      if (!active) return;
      try {
        options.onData(JSON.parse(rowsJson) as T[]);
      } catch (error) {
        const message = errorMessage(error);
        captureOperationalError(error, {
          operation: "database_live_query_decode",
        });
        options.onError?.(message);
      }
    },
    onError(message) {
      if (!active) return;
      captureOperationalError(new Error(message), {
        operation: "database_live_query",
      });
      options.onError?.(message);
    },
  };

  let subscriptionId: string;
  try {
    subscriptionId = getBridge().subscribe(
      sql,
      JSON.stringify(params),
      listener,
    );
  } catch (error) {
    captureOperationalError(error, {
      operation: "database_live_query_subscribe",
    });
    throw error;
  }

  return async () => {
    if (!active) return;
    active = false;
    try {
      getBridge().unsubscribe(subscriptionId);
    } catch (error) {
      captureOperationalError(error, {
        operation: "database_live_query_unsubscribe",
        level: "warning",
      });
    }
  };
}

export type MobileSyncStatus = {
  configured: boolean;
  running: boolean;
  has_unsent_changes: boolean | null;
  last_sync_at_ms: number | null;
  last_error: string | null;
  consecutive_failures: number;
};

export async function configureE2eeReplica({
  workspaceId,
  witnessEndpoint,
  witnessAccessToken,
  recoveryKeyCode,
}: {
  workspaceId: string;
  witnessEndpoint: string;
  witnessAccessToken: string;
  recoveryKeyCode: string;
}): Promise<"configured" | "account_mismatch"> {
  try {
    const result = getBridge().configureE2eeReplica(
      workspaceId,
      witnessEndpoint,
      witnessAccessToken,
      recoveryKeyCode,
    );
    if (result !== "configured" && result !== "account_mismatch") {
      throw new Error(`Unexpected replica configuration result: ${result}`);
    }
    return result;
  } catch (error) {
    captureOperationalError(error, {
      operation: "database_replica_configure",
    });
    throw error;
  }
}

export async function startSync(): Promise<void> {
  try {
    getBridge().startCloudsync();
  } catch (error) {
    captureOperationalError(error, { operation: "database_sync_start" });
    throw error;
  }
}

export async function stopSync(): Promise<void> {
  try {
    getBridge().stopCloudsync();
  } catch (error) {
    captureOperationalError(error, { operation: "database_sync_stop" });
    throw error;
  }
}

export async function syncNow(): Promise<void> {
  try {
    getBridge().cloudsyncSyncNow();
  } catch (error) {
    captureOperationalError(error, { operation: "database_sync_now" });
    throw error;
  }
}

export async function getSyncStatus(): Promise<MobileSyncStatus> {
  try {
    return JSON.parse(getBridge().cloudsyncStatus()) as MobileSyncStatus;
  } catch (error) {
    captureOperationalError(error, {
      operation: "database_sync_status",
    });
    throw error;
  }
}

export const mobileLiveQueryClient: LiveQueryClient = {
  execute,
  subscribe,
};

export const mobileTransactionClient: TransactionClient = {
  executeTransaction,
};
