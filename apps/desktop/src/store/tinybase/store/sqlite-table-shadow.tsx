import type { Row, Table } from "tinybase/with-schemas";

import type { Schemas, Store } from "./main";
import { registerSaveHandler } from "./save";
import { markSqliteShadowReady } from "./sqlite-shadow-cutover";

import { liveQueryClient } from "~/db";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

type MainTableId = keyof Schemas[0] & string;
export type MainTableRow<K extends MainTableId> = Row<Schemas[0], K>;

export type SqliteTableShadowConfig<K extends MainTableId, TSqliteRow> = {
  label: string;
  tableId: K;
  selectSql: string;
  fromSqlite: (row: TSqliteRow) => MainTableRow<K>;
  isDeleted?: (row: TSqliteRow) => boolean;
  normalize: (row: MainTableRow<K>) => MainTableRow<K>;
  persist: (
    rows: Array<[string, MainTableRow<K>]>,
    deletedIds: string[],
  ) => Promise<void>;
};

export function SqliteTableShadow<K extends MainTableId, TSqliteRow>({
  config,
  store,
}: {
  config: SqliteTableShadowConfig<K, TSqliteRow>;
  store: Store;
}) {
  useMountEffect(() => {
    let disposed = false;
    let tableListenerId: string | undefined;
    let unsubscribe: (() => Promise<void>) | undefined;
    let applyingSqliteSnapshot = false;
    let initialized = false;
    let requested = false;
    let syncing = false;
    let pendingSqliteRows: TSqliteRow[] | undefined;
    let persistedFingerprints = new Map<string, string>();
    let syncError: unknown;
    let idleWaiters: Array<() => void> = [];
    let readyReported = false;

    const installTableListener = () => {
      if (tableListenerId) return;
      tableListenerId = String(
        store.addTableListener(config.tableId, schedule),
      );
    };

    const applySqliteRows = (rows: TSqliteRow[]) => {
      const deletedIds = new Set(
        rows
          .filter((row) => config.isDeleted?.(row) ?? false)
          .map(getSqliteRowId),
      );
      const sqliteRows = Object.fromEntries(
        rows
          .filter((row) => !deletedIds.has(getSqliteRowId(row)))
          .map((row) => {
            const mapped = config.fromSqlite(row);
            return [getSqliteRowId(row), mapped];
          }),
      ) as Table<Schemas[0], K>;
      const legacyRows = Object.fromEntries(
        Object.entries(store.getTable(config.tableId)).filter(
          ([id]) => !deletedIds.has(id),
        ),
      ) as Table<Schemas[0], K>;
      const nextRows = initialized
        ? sqliteRows
        : { ...legacyRows, ...sqliteRows };

      applyingSqliteSnapshot = true;
      try {
        store.transaction(() => {
          for (const id of store.getRowIds(config.tableId)) {
            if (!(id in nextRows)) {
              store.delRow(config.tableId, id);
            }
          }
          for (const [id, row] of Object.entries(nextRows)) {
            store.setRow(config.tableId, id, row);
          }
        });
        persistedFingerprints = fingerprintTable(sqliteRows, config.normalize);
        initialized = true;
      } finally {
        applyingSqliteSnapshot = false;
      }

      installTableListener();
      if (Object.keys(nextRows).length !== Object.keys(sqliteRows).length) {
        schedule();
      } else {
        reportReady();
      }
    };

    const sync = async () => {
      if (syncing || disposed) return;
      syncing = true;
      syncError = undefined;

      try {
        while (requested && !disposed) {
          requested = false;
          const rows = Object.entries(store.getTable(config.tableId));
          const fingerprints = fingerprintEntries(rows, config.normalize);
          const changedRows = rows.filter(
            ([id]) => persistedFingerprints.get(id) !== fingerprints.get(id),
          );
          const deletedIds = [...persistedFingerprints.keys()].filter(
            (id) => !fingerprints.has(id),
          );

          await config.persist(changedRows, deletedIds);
          persistedFingerprints = fingerprints;
        }
      } catch (error) {
        syncError = error;
        console.error(`[${config.label}]`, error);
      } finally {
        syncing = false;
        if (pendingSqliteRows && !disposed) {
          const rows = pendingSqliteRows;
          pendingSqliteRows = undefined;
          applySqliteRows(rows);
        }
        if (requested && !disposed) {
          queueMicrotask(sync);
        } else {
          reportReady();
          const waiters = idleWaiters;
          idleWaiters = [];
          waiters.forEach((resolve) => resolve());
        }
      }
    };

    function schedule() {
      if (applyingSqliteSnapshot || disposed) return;
      requested = true;
      queueMicrotask(sync);
    }

    function reportReady() {
      if (
        readyReported ||
        !initialized ||
        syncing ||
        requested ||
        pendingSqliteRows ||
        syncError
      ) {
        return;
      }
      readyReported = true;
      void markSqliteShadowReady(config.tableId).catch((error) => {
        console.error(`[${config.label}] failed to mark SQLite cutover`, error);
      });
    }

    const flush = async () => {
      schedule();
      while (syncing || requested) {
        await new Promise<void>((resolve) => idleWaiters.push(resolve));
      }
      if (syncError) {
        throw syncError;
      }
    };

    const unregisterSaveHandler = registerSaveHandler(
      `sqlite-shadow:${config.tableId}`,
      flush,
    );

    void liveQueryClient
      .subscribe<TSqliteRow>(config.selectSql, [], {
        onData: (rows) => {
          if (disposed) return;
          if (initialized && (syncing || requested)) {
            pendingSqliteRows = rows;
            return;
          }
          applySqliteRows(rows);
        },
        onError: (error) => {
          console.error(`[${config.label}]`, error);
        },
      })
      .then((dispose) => {
        if (disposed) {
          void dispose();
          return;
        }
        unsubscribe = dispose;
      })
      .catch((error) => {
        console.error(`[${config.label}]`, error);
      });

    return () => {
      disposed = true;
      if (tableListenerId) {
        store.delListener(tableListenerId);
      }
      unregisterSaveHandler();
      void unsubscribe?.();
    };
  });

  return null;
}

function getSqliteRowId(row: unknown): string {
  if (
    !row ||
    typeof row !== "object" ||
    !("id" in row) ||
    typeof row.id !== "string"
  ) {
    throw new Error("SQLite shadow query returned a row without a string id");
  }
  return row.id;
}

function fingerprintTable<K extends MainTableId>(
  table: Table<Schemas[0], K>,
  normalize: (row: MainTableRow<K>) => MainTableRow<K>,
) {
  return fingerprintEntries(Object.entries(table), normalize);
}

function fingerprintEntries<K extends MainTableId>(
  rows: Array<[string, MainTableRow<K>]>,
  normalize: (row: MainTableRow<K>) => MainTableRow<K>,
) {
  return new Map(rows.map(([id, row]) => [id, JSON.stringify(normalize(row))]));
}
