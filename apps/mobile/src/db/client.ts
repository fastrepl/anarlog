import {
  addDatabaseChangeListener,
  openDatabaseAsync,
  type SQLiteBindValue,
  type SQLiteDatabase,
} from "expo-sqlite";

import type {
  LiveQueryClient,
  Row,
  TransactionClient,
  TransactionStatement,
  Unsubscribe,
} from "@hypr/db-runtime";

import { migrate } from "@/db/migrations";
import { captureOperationalError } from "@/lib/error-reporting";
import { id } from "@/lib/ids";

const READ_TABLE_RE = /\b(?:from|join)\s+([^\s(,;]+)/gi;
const WRITE_TABLE_RE =
  /\b(?:insert\s+(?:or\s+\w+\s+)?into|update(?:\s+or\s+\w+)?|delete\s+from)\s+([^\s(,;]+)/gi;

function normalizeIdentifier(token: string): string | null {
  const last = token.split(".").pop() ?? token;
  const cleaned = last
    .replace(/^[`"\[]+/, "")
    .replace(/[`"\])]+$/, "")
    .toLowerCase();
  return /^[a-z_][a-z0-9_]*$/.test(cleaned) ? cleaned : null;
}

function scanTables(sql: string, pattern: RegExp): Set<string> {
  const tables = new Set<string>();
  for (const match of sql.matchAll(pattern)) {
    const table = normalizeIdentifier(match[1]);
    if (table) {
      tables.add(table);
    }
  }
  return tables;
}

function mapParam(value: unknown): SQLiteBindValue {
  if (value === null || value === undefined) {
    return null;
  }
  if (typeof value === "boolean") {
    return value ? 1 : 0;
  }
  if (typeof value === "number" || typeof value === "string") {
    return value;
  }
  if (value instanceof Uint8Array) {
    return value;
  }
  return JSON.stringify(value);
}

function mapParams(params: unknown[] | undefined): SQLiteBindValue[] {
  return (params ?? []).map(mapParam);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

type Subscription = {
  sql: string;
  params: SQLiteBindValue[];
  tables: Set<string>;
  active: boolean;
  reportedError?: string;
  onData: (rows: Row[]) => void;
  onError?: (error: string) => void;
};

const subscriptions = new Set<Subscription>();
const pendingTables = new Set<string>();
let flushTimer: ReturnType<typeof setTimeout> | null = null;
let dbPromise: Promise<SQLiteDatabase> | null = null;
// Single shared connection: every read and write is serialized through this
// queue so no SELECT can interleave into an open BEGIN IMMEDIATE.
let dbQueue: Promise<unknown> = Promise.resolve();

function enqueue<T>(operation: () => Promise<T>): Promise<T> {
  const run = dbQueue.then(operation);
  dbQueue = run.catch(() => {});
  return run;
}

function getDb(): Promise<SQLiteDatabase> {
  if (!dbPromise) {
    dbPromise = openDb();
  }
  return dbPromise;
}

async function openDb(): Promise<SQLiteDatabase> {
  const db = await openDatabaseAsync("anarlog.db", {
    enableChangeListener: true,
  });
  try {
    await db.execAsync("PRAGMA journal_mode = WAL");
  } catch (error) {
    captureOperationalError(error, {
      operation: "database_pragma",
      level: "warning",
      tags: { pragma: "journal_mode_wal" },
    });
  }
  try {
    await db.execAsync("PRAGMA foreign_keys = ON");
  } catch (error) {
    captureOperationalError(error, {
      operation: "database_pragma",
      level: "warning",
      tags: { pragma: "foreign_keys_on" },
    });
  }
  try {
    await migrate(db);
  } catch (error) {
    captureOperationalError(error, {
      operation: "database_migrate",
    });
    throw error;
  }
  try {
    await ensureWorkspaceBinding(db);
  } catch (error) {
    captureOperationalError(error, {
      operation: "database_workspace_binding",
    });
    throw error;
  }
  try {
    addDatabaseChangeListener((event) => {
      if (event.tableName) {
        emitTableChange(event.tableName);
      }
    });
  } catch {
    // Unsupported platform (e.g. web); write-path invalidation still applies.
  }
  return db;
}

// Mirrors crates/db-app ensure_cloudsync_workspace_binding: every writing
// client needs the binding row so insert SQL can resolve workspace_id.
async function ensureWorkspaceBinding(db: SQLiteDatabase): Promise<void> {
  const rows = await db.getAllAsync<{ value_json: string }>(
    "SELECT value_json FROM app_settings WHERE id = 'cloudsync_workspace_binding'",
  );
  if (rows.length > 0) return;
  await db.runAsync(
    "INSERT INTO app_settings (id, value_json) VALUES ('cloudsync_workspace_binding', ?)",
    [JSON.stringify({ workspace_id: id(), account_user_id: null })],
  );
}

function emitTableChange(table: string): void {
  pendingTables.add(table.toLowerCase());
  if (flushTimer === null) {
    flushTimer = setTimeout(flushTableChanges, 10);
  }
}

function emitWrittenTables(sql: string): void {
  for (const table of scanTables(sql, WRITE_TABLE_RE)) {
    emitTableChange(table);
  }
}

function flushTableChanges(): void {
  flushTimer = null;
  const changed = new Set(pendingTables);
  pendingTables.clear();
  for (const subscription of subscriptions) {
    let affected = false;
    for (const table of subscription.tables) {
      if (changed.has(table)) {
        affected = true;
        break;
      }
    }
    if (affected) {
      void refresh(subscription);
    }
  }
}

async function refresh(subscription: Subscription): Promise<void> {
  try {
    const db = await getDb();
    const rows = await enqueue(() =>
      db.getAllAsync<Row>(subscription.sql, subscription.params),
    );
    if (subscription.active) {
      subscription.reportedError = undefined;
      subscription.onData(rows);
    }
  } catch (error) {
    const message = errorMessage(error);
    if (subscription.reportedError !== message) {
      subscription.reportedError = message;
      captureOperationalError(error, {
        operation: "database_live_query_refresh",
      });
    }
    if (subscription.active) {
      subscription.onError?.(message);
    }
  }
}

async function execute<T = Row>(sql: string, params?: unknown[]): Promise<T[]> {
  const db = await getDb();
  const rows = await enqueue(() => db.getAllAsync<T>(sql, mapParams(params)));
  emitWrittenTables(sql);
  return rows;
}

async function runTransaction(
  statements: TransactionStatement[],
): Promise<number[]> {
  const db = await getDb();
  await db.execAsync("BEGIN IMMEDIATE");
  const affected: number[] = [];
  try {
    for (let i = 0; i < statements.length; i++) {
      const statement = statements[i];
      const result = await db.runAsync(
        statement.sql,
        mapParams(statement.params),
      );
      const expected = statement.expectedRowsAffected;
      if (expected != null && result.changes !== expected) {
        throw new Error(
          `transaction statement ${i} affected ${result.changes} rows; expected ${expected}`,
        );
      }
      affected.push(result.changes);
    }
    await db.execAsync("COMMIT");
  } catch (error) {
    try {
      await db.execAsync("ROLLBACK");
    } catch (rollbackError) {
      captureOperationalError(rollbackError, {
        operation: "database_transaction_rollback",
        level: "warning",
      });
    }
    throw error;
  } finally {
    // Also after ROLLBACK: hook events consumed mid-transaction must not
    // leave subscribers on phantom state — re-emit so they re-query.
    for (const statement of statements) {
      emitWrittenTables(statement.sql);
    }
  }
  return affected;
}

function executeTransaction(
  statements: TransactionStatement[],
): Promise<number[]> {
  return enqueue(() => runTransaction(statements));
}

async function subscribe<T = Row>(
  sql: string,
  params: unknown[],
  options: { onData: (rows: T[]) => void; onError?: (error: string) => void },
): Promise<Unsubscribe> {
  const db = await getDb();
  const boundParams = mapParams(params);
  const tables = scanTables(sql, READ_TABLE_RE);
  if (tables.size === 0) {
    console.warn(
      `[mobile-db] live query subscription is non-reactive for SQL "${sql}": query has no reactive dependencies`,
    );
  }
  let reportedError: string | undefined;
  try {
    const rows = await enqueue(() => db.getAllAsync<T>(sql, boundParams));
    options.onData(rows);
  } catch (error) {
    reportedError = errorMessage(error);
    captureOperationalError(error, {
      operation: "database_live_query_subscribe",
    });
    options.onError?.(reportedError);
  }
  const subscription: Subscription = {
    sql,
    params: boundParams,
    tables,
    active: true,
    reportedError,
    onData: options.onData as (rows: Row[]) => void,
    onError: options.onError,
  };
  if (tables.size > 0) {
    subscriptions.add(subscription);
  }
  return async () => {
    subscription.active = false;
    subscriptions.delete(subscription);
  };
}

export const mobileLiveQueryClient: LiveQueryClient = {
  execute,
  subscribe,
};

export const mobileTransactionClient: TransactionClient = {
  executeTransaction,
};
