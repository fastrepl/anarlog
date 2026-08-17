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
import {
  requestReplicaCredentials,
  type E2eeRecoveryKeyIdentity,
} from "@/sync/replica-credentials";

export type { E2eeRecoveryKeyIdentity } from "@/sync/replica-credentials";

let bridge: MobileDbBridgeLike | null = null;

function filePath(uri: string): string {
  return decodeURIComponent(uri.replace(/^file:\/\//, ""));
}

function getBridge(): MobileDbBridgeLike {
  if (!bridge) {
    const databaseDirectory = new Directory(Paths.document, "SQLite");
    databaseDirectory.create({ intermediates: true, idempotent: true });
    const databaseUri = new File(databaseDirectory, "anarlog.db").uri;
    const databasePath = filePath(databaseUri);
    bridge = MobileDbBridge.open(databasePath, "disabled");
    bridge.configureAttachmentStorage(
      filePath(Paths.document.uri),
      filePath(Paths.cache.uri),
    );
  }
  return bridge;
}

export async function restoreAttachment(
  input: {
    sessionId: string;
    attachmentId: string;
    objectId: string;
    objectKey: string;
    signedUrl: string;
    supabaseUrl: string;
    ciphertextSha256: string;
    ciphertextSizeBytes: number;
    formatVersion: number;
  },
  signal?: AbortSignal,
): Promise<{
  attachmentId: string;
  sessionId: string;
  relativePath: string;
  sizeBytes: number;
  sha256: string;
}> {
  try {
    return JSON.parse(
      await getBridge().restoreAttachment(
        JSON.stringify(input),
        signal ? { signal } : undefined,
      ),
    ) as {
      attachmentId: string;
      sessionId: string;
      relativePath: string;
      sizeBytes: number;
      sha256: string;
    };
  } catch (error) {
    captureOperationalError(error, { operation: "attachment_restore" });
    throw error;
  }
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

export async function generateE2eeRecoveryKey(): Promise<string> {
  try {
    return getBridge().generateE2eeRecoveryKey();
  } catch (error) {
    captureOperationalError(error, {
      operation: "database_recovery_key_generate",
    });
    throw error;
  }
}

export async function inspectE2eeRecoveryKey(
  recoveryKeyCode: string,
): Promise<E2eeRecoveryKeyIdentity> {
  try {
    const value: unknown = JSON.parse(
      getBridge().inspectE2eeRecoveryKey(recoveryKeyCode),
    );
    if (!value || typeof value !== "object") {
      throw new Error("Unexpected recovery key identity");
    }
    const candidate = value as Record<string, unknown>;
    if (
      typeof candidate.keyId !== "string" ||
      !/^[A-Za-z0-9_-]{22}$/.test(candidate.keyId) ||
      typeof candidate.memberPublicKey !== "string" ||
      !/^[A-Za-z0-9_-]{43}$/.test(candidate.memberPublicKey)
    ) {
      throw new Error("Unexpected recovery key identity");
    }
    return {
      keyId: candidate.keyId,
      memberPublicKey: candidate.memberPublicKey,
    };
  } catch (error) {
    captureOperationalError(error, {
      operation: "database_recovery_key_inspect",
    });
    throw error;
  }
}

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

export async function bootstrapE2eeReplica({
  apiUrl,
  accessToken,
  accountUserId,
  recoveryKeyCode,
  device,
}: {
  apiUrl: string;
  accessToken: string;
  accountUserId: string;
  recoveryKeyCode: string;
  device?: { fingerprint?: string | null; name?: string | null };
}): Promise<"configured" | "account_mismatch"> {
  try {
    const identity = await inspectE2eeRecoveryKey(recoveryKeyCode);
    const credentials = await requestReplicaCredentials({
      apiUrl,
      accessToken,
      accountUserId,
      identity,
      device,
    });
    const result = await configureE2eeReplica({
      workspaceId: credentials.workspaceId,
      witnessEndpoint: new URL(
        `/sync/e2ee/witness/${encodeURIComponent(credentials.workspaceId)}`,
        apiUrl,
      ).toString(),
      witnessAccessToken: accessToken,
      recoveryKeyCode,
    });
    if (result === "configured") {
      await startSync();
    }
    return result;
  } catch (error) {
    captureOperationalError(error, {
      operation: "database_replica_bootstrap",
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
