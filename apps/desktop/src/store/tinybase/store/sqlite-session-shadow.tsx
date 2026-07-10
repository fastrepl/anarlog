import { useQuery } from "@tanstack/react-query";

import { eq, sessionDocuments, sessions, sql } from "@hypr/db";
import { md2json } from "@hypr/editor/markdown";
import {
  getLegacyImportReport,
  type LegacyImportReport,
} from "@hypr/plugin-db";
import type { SessionStorage } from "@hypr/store";

import type { Store } from "./main";

import { db, liveQueryClient } from "~/db";
import { getSessionEvent } from "~/session/utils";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

type TinyBaseSessionRow = Partial<SessionStorage>;
type NewSessionRow = typeof sessions.$inferInsert;
type NewSessionDocumentRow = typeof sessionDocuments.$inferInsert;

type SqliteSessionSnapshotRow = {
  id: string;
  owner_user_id: string;
  title: string;
  created_at: string;
  event_json: string;
  folder_path: string;
  raw_md: string;
  raw_body_format: string;
};

const SESSION_SNAPSHOT_SQL = `
  SELECT
    sessions.id,
    sessions.owner_user_id,
    sessions.title,
    sessions.created_at,
    sessions.event_json,
    sessions.folder_path,
    COALESCE(session_documents.body, '') AS raw_md,
    COALESCE(session_documents.body_format, 'prosemirror_json') AS raw_body_format
  FROM sessions
  LEFT JOIN session_documents
    ON session_documents.id = sessions.id
    AND session_documents.kind = 'note'
    AND session_documents.deleted_at IS NULL
  WHERE sessions.deleted_at IS NULL
  ORDER BY sessions.created_at, sessions.id
`;

export function SqliteSessionShadow({
  enabled,
  store,
}: {
  enabled: boolean;
  store: Store;
}) {
  const reportQuery = useQuery({
    queryKey: ["legacy-import-report"],
    queryFn: getLegacyImportReport,
    enabled,
    refetchInterval: (query) => {
      const report = query.state.data as LegacyImportReport | undefined;
      const status = report?.latestRun?.status;
      return status && status !== "running" ? false : 250;
    },
  });

  if (
    !enabled ||
    !reportQuery.data?.state.parityVerified ||
    reportQuery.data.latestRun?.status !== "completed"
  ) {
    return null;
  }

  return <MountedSqliteSessionShadow store={store} />;
}

function MountedSqliteSessionShadow({ store }: { store: Store }) {
  useMountEffect(() => {
    let disposed = false;
    let tableListenerId: string | undefined;
    let unsubscribe: (() => Promise<void>) | undefined;
    let applyingSqliteSnapshot = false;
    let initialized = false;
    let requested = false;
    let syncing = false;
    let pendingSqliteRows: SqliteSessionSnapshotRow[] | undefined;
    let persistedFingerprints = new Map<string, string>();

    const installTableListener = () => {
      if (tableListenerId) return;
      tableListenerId = String(store.addTableListener("sessions", schedule));
    };

    const applySqliteRows = (rows: SqliteSessionSnapshotRow[]) => {
      const sqliteRows = Object.fromEntries(
        rows.map((row) => [row.id, fromSqliteSessionRow(row)]),
      );
      const nextRows = initialized
        ? sqliteRows
        : { ...store.getTable("sessions"), ...sqliteRows };

      applyingSqliteSnapshot = true;
      try {
        store.setTable("sessions", nextRows);
        persistedFingerprints = fingerprintTable(sqliteRows);
        initialized = true;
      } finally {
        applyingSqliteSnapshot = false;
      }

      installTableListener();
      if (Object.keys(nextRows).length !== Object.keys(sqliteRows).length) {
        schedule();
      }
    };

    const sync = async () => {
      if (syncing || disposed) return;
      syncing = true;

      try {
        while (requested && !disposed) {
          requested = false;
          const rows = Object.entries(store.getTable("sessions")) as Array<
            [string, TinyBaseSessionRow]
          >;
          const fingerprints = fingerprintEntries(rows);
          const changedRows = rows.filter(
            ([id]) => persistedFingerprints.get(id) !== fingerprints.get(id),
          );
          const deletedIds = [...persistedFingerprints.keys()].filter(
            (id) => !fingerprints.has(id),
          );

          await persistSessionChanges(changedRows, deletedIds);
          persistedFingerprints = fingerprints;
        }
      } catch (error) {
        console.error("[SqliteSessionShadow]", error);
      } finally {
        syncing = false;
        if (pendingSqliteRows && !disposed) {
          const rows = pendingSqliteRows;
          pendingSqliteRows = undefined;
          applySqliteRows(rows);
        }
      }
    };

    function schedule() {
      if (applyingSqliteSnapshot || disposed) return;
      requested = true;
      queueMicrotask(sync);
    }

    void liveQueryClient
      .subscribe<SqliteSessionSnapshotRow>(SESSION_SNAPSHOT_SQL, [], {
        onData: (rows) => {
          if (disposed) return;
          if (initialized && (syncing || requested)) {
            pendingSqliteRows = rows;
            return;
          }
          applySqliteRows(rows);
        },
        onError: (error) => {
          console.error("[SqliteSessionShadow]", error);
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
        console.error("[SqliteSessionShadow]", error);
      });

    return () => {
      disposed = true;
      if (tableListenerId) {
        store.delListener(tableListenerId);
      }
      void unsubscribe?.();
    };
  });

  return null;
}

async function persistSessionChanges(
  rows: Array<[string, TinyBaseSessionRow]>,
  deletedIds: string[],
): Promise<void> {
  for (const chunk of chunks(rows, 40)) {
    const sessionRows = chunk.map(([id, row]) => toSqliteSessionRow(id, row));
    const documentRows = chunk.map(([id, row]) =>
      toSqliteSessionDocumentRow(id, row),
    );

    await db
      .insert(sessions)
      .values(sessionRows)
      .onConflictDoUpdate({
        target: sessions.id,
        set: {
          ownerUserId: sql`excluded.owner_user_id`,
          title: sql`excluded.title`,
          status: "active",
          startedAt: sql`excluded.started_at`,
          endedAt: sql`excluded.ended_at`,
          externalEventId: sql`excluded.external_event_id`,
          seriesId: sql`excluded.series_id`,
          eventJson: sql`excluded.event_json`,
          folderPath: sql`excluded.folder_path`,
          deletedAt: null,
          updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`,
        },
      });

    await db
      .insert(sessionDocuments)
      .values(documentRows)
      .onConflictDoUpdate({
        target: sessionDocuments.id,
        set: {
          bodyFormat: "prosemirror_json",
          body: sql`excluded.body`,
          updatedBy: sql`excluded.updated_by`,
          deletedAt: null,
          updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`,
        },
      });
  }

  const deletedAt = new Date().toISOString();
  for (const id of deletedIds) {
    await db
      .update(sessions)
      .set({ deletedAt, updatedAt: deletedAt })
      .where(eq(sessions.id, id));
  }
}

export function toSqliteSessionRow(
  id: string,
  row: TinyBaseSessionRow,
): NewSessionRow {
  const normalized = normalizeTinyBaseSessionRow(row);
  const event = getSessionEvent({ event_json: normalized.event_json });
  const now = new Date().toISOString();

  return {
    id,
    workspaceId: "",
    ownerUserId: normalized.user_id,
    title: normalized.title,
    kind: "meeting",
    status: "active",
    createdAt: normalized.created_at || now,
    updatedAt: now,
    startedAt: event?.started_at ?? "",
    endedAt: event?.ended_at ?? "",
    timezone: "",
    language: "",
    eventId: "",
    externalEventId: event?.tracking_id ?? "",
    externalProvider: "",
    seriesId: event?.recurrence_series_id ?? "",
    sourceAppsJson: "[]",
    eventJson: normalized.event_json,
    folderPath: normalized.folder_id,
    slug: "",
    metadataJson: "{}",
    deletedAt: null,
  };
}

export function toSqliteSessionDocumentRow(
  id: string,
  row: TinyBaseSessionRow,
): NewSessionDocumentRow {
  const normalized = normalizeTinyBaseSessionRow(row);
  const now = new Date().toISOString();

  return {
    id,
    workspaceId: "",
    sessionId: id,
    kind: "note",
    templateId: "",
    title: "",
    bodyFormat: "prosemirror_json",
    body: normalized.raw_md,
    sourceHash: "",
    generationMetadataJson: "{}",
    sortOrder: 0,
    createdBy: normalized.user_id,
    updatedBy: normalized.user_id,
    createdAt: normalized.created_at || now,
    updatedAt: now,
    deletedAt: null,
  };
}

export function fromSqliteSessionRow(
  row: SqliteSessionSnapshotRow,
): TinyBaseSessionRow {
  return {
    user_id: row.owner_user_id,
    created_at: row.created_at,
    folder_id: row.folder_path,
    event_json: row.event_json,
    title: row.title,
    raw_md: decodeSessionBody(row.raw_md, row.raw_body_format),
  };
}

function decodeSessionBody(body: string, format: string): string {
  if (!body || format !== "markdown") return body;

  try {
    return JSON.stringify(md2json(body));
  } catch (error) {
    console.error(
      "[SqliteSessionShadow] failed to decode Markdown note",
      error,
    );
    return body;
  }
}

function normalizeTinyBaseSessionRow(row: TinyBaseSessionRow) {
  return {
    user_id: typeof row.user_id === "string" ? row.user_id : "",
    created_at: typeof row.created_at === "string" ? row.created_at : "",
    folder_id: typeof row.folder_id === "string" ? row.folder_id : "",
    event_json: typeof row.event_json === "string" ? row.event_json : "",
    title: typeof row.title === "string" ? row.title : "",
    raw_md: typeof row.raw_md === "string" ? row.raw_md : "",
  };
}

function fingerprintTable(table: Record<string, TinyBaseSessionRow>) {
  return fingerprintEntries(Object.entries(table));
}

function fingerprintEntries(rows: Array<[string, TinyBaseSessionRow]>) {
  return new Map(
    rows.map(([id, row]) => [
      id,
      JSON.stringify(normalizeTinyBaseSessionRow(row)),
    ]),
  );
}

function chunks<T>(items: T[], size: number): T[][] {
  const result: T[][] = [];
  for (let index = 0; index < items.length; index += size) {
    result.push(items.slice(index, index + size));
  }
  return result;
}
