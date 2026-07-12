import { Channel, invoke } from "@tauri-apps/api/core";

import type {
  LegacyCleanupResult,
  LegacyCleanupStatus,
  LegacyImportReport,
  SubscriptionRegistration,
} from "./bindings.gen";

export type {
  LegacyCleanupResult,
  LegacyCleanupStatus,
  LegacyImportReport,
} from "./bindings.gen";

export type TransactionStatement = {
  sql: string;
  params: unknown[];
  expectedRowsAffected?: number;
};

export type QueryEvent<T = Record<string, unknown>> =
  | { event: "result"; data: T[] }
  | { event: "error"; data: string };

// Generic query path: returns named object rows for app-level SQL consumers.
export async function execute<T = Record<string, unknown>>(
  sql: string,
  params: unknown[] = [],
): Promise<T[]> {
  return invoke("plugin:db|execute", { sql, params });
}

export async function executeTransaction(
  statements: TransactionStatement[],
): Promise<number[]> {
  return invoke("plugin:db|execute_transaction", { statements });
}

// Drizzle proxy path: returns raw positional rows in sqlite-proxy format.
export async function executeProxy(
  sql: string,
  params: unknown[] = [],
  method: "run" | "all" | "get" | "values",
): Promise<{ rows: unknown[] }> {
  return invoke("plugin:db|execute_proxy", { sql, params, method });
}

export async function getLegacyImportReport(): Promise<LegacyImportReport> {
  return invoke("plugin:db|get_legacy_import_report");
}

export async function getLegacyCleanupStatus(): Promise<LegacyCleanupStatus> {
  return invoke("plugin:db|get_legacy_cleanup_status");
}

export async function cleanupLegacyFiles(): Promise<LegacyCleanupResult> {
  return invoke("plugin:db|cleanup_legacy_files");
}

export async function runLegacyImport(dryRun = false): Promise<string> {
  return invoke("plugin:db|run_legacy_import", { dryRun });
}

export async function subscribe<T = Record<string, unknown>>(
  sql: string,
  params: unknown[],
  options: {
    onData: (rows: T[]) => void;
    onError?: (error: string) => void;
  },
): Promise<() => Promise<void>> {
  const channel = new Channel<QueryEvent<T>>();

  channel.onmessage = (event) => {
    if (event.event === "result") {
      options.onData(event.data);
      return;
    }

    options.onError?.(event.data);
  };

  const registration: SubscriptionRegistration = await invoke(
    "plugin:db|subscribe",
    {
      sql,
      params,
      onEvent: channel,
    },
  );

  if (registration.analysis.kind === "non_reactive") {
    console.warn(
      `[plugin-db] live query subscription is non-reactive for SQL "${sql}": ${registration.analysis.data.reason}`,
    );
  }

  return async () => {
    channel.onmessage = () => {};
    await invoke("plugin:db|unsubscribe", { subscriptionId: registration.id });
  };
}
