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

export const mobileLiveQueryClient: LiveQueryClient = {
  execute,
  subscribe,
};

export const mobileTransactionClient: TransactionClient = {
  executeTransaction,
};
