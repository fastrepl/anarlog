import { type SearchDocument, commands as tantivy } from "@hypr/plugin-tantivy";

import {
  createHumanSearchableContent,
  createSessionSearchableContent,
} from "./content";
import {
  extractPlainText,
  flattenTranscript,
  getSessionSearchTimestamp,
  mergeContent,
  toEpochMs,
  toTrimmedString,
} from "./utils";

import { liveQueryClient } from "~/db";

type SessionSearchSqlRow = {
  id: string;
  created_at: string;
  event_json: string;
  title: string;
  raw_body: string;
  enhanced_notes_json: string;
  transcripts_json: string;
};

type HumanSearchSqlRow = {
  id: string;
  name: string;
  email: string;
  job_title: string;
  linkedin_username: string;
  created_at: string;
  memo: string;
};

type OrganizationSearchSqlRow = {
  id: string;
  name: string;
  created_at: string;
};

type SourceKey = "sessions" | "humans" | "organizations";

type InitialGate = {
  promise: Promise<void>;
  resolve: () => void;
  reject: (error: Error) => void;
  settled: boolean;
};

const SESSION_SEARCH_SQL = `
  SELECT
    session.id,
    session.created_at,
    session.event_json,
    session.title,
    COALESCE(
      (
        SELECT document.body
        FROM session_documents AS document
        WHERE document.id = session.id
          AND document.session_id = session.id
          AND document.kind = 'note'
          AND document.deleted_at IS NULL
        LIMIT 1
      ),
      (
        SELECT document.body
        FROM session_documents AS document
        WHERE document.session_id = session.id
          AND document.kind = 'note'
          AND document.deleted_at IS NULL
        ORDER BY document.created_at, document.id
        LIMIT 1
      ),
      ''
    ) AS raw_body,
    COALESCE((
      SELECT json_group_array(document.body)
      FROM session_documents AS document
      WHERE document.session_id = session.id
        AND document.kind IN ('summary', 'template_output')
        AND document.deleted_at IS NULL
    ), '[]') AS enhanced_notes_json,
    COALESCE((
      SELECT json_group_array(transcript.words_json)
      FROM transcripts AS transcript
      WHERE transcript.session_id = session.id
        AND transcript.deleted_at IS NULL
    ), '[]') AS transcripts_json
  FROM sessions AS session
  WHERE session.deleted_at IS NULL
  ORDER BY session.id
`;

const HUMAN_SEARCH_SQL = `
  SELECT
    id,
    name,
    email,
    job_title,
    linkedin_username,
    created_at,
    memo
  FROM humans
  WHERE deleted_at IS NULL
  ORDER BY id
`;

const ORGANIZATION_SEARCH_SQL = `
  SELECT id, name, created_at
  FROM organizations
  WHERE deleted_at IS NULL
  ORDER BY id
`;

export function createSearchIndexSync() {
  const snapshots = new Map<SourceKey, SearchDocument[]>();
  const gates = new Map<SourceKey, InitialGate>([
    ["sessions", createInitialGate()],
    ["humans", createInitialGate()],
    ["organizations", createInitialGate()],
  ]);
  const initialSnapshots = Promise.all(
    Array.from(gates.values(), (gate) => gate.promise),
  );
  void initialSnapshots.catch(() => {});
  const unsubscribers: Array<() => Promise<void>> = [];
  let indexedDocuments = new Map<string, SearchDocument>();
  let reconcileQueue = Promise.resolve();
  let ready = false;
  let stopped = false;
  let cancelInitialWait = () => {};
  const cancelled = new Promise<void>((resolve) => {
    cancelInitialWait = resolve;
  });

  const scheduleReconcile = () => {
    reconcileQueue = reconcileQueue
      .then(async () => {
        if (stopped) return;
        indexedDocuments = await reconcileDocuments(
          indexedDocuments,
          collectDocuments(snapshots),
        );
      })
      .catch((error) => {
        console.error("Failed to update search index:", error);
      });
    return reconcileQueue;
  };

  const handleRows = <Row>(
    key: SourceKey,
    rows: Row[],
    mapRows: (rows: Row[]) => SearchDocument[],
  ) => {
    snapshots.set(key, mapRows(rows));
    const gate = gates.get(key)!;
    if (!gate.settled) {
      gate.settled = true;
      gate.resolve();
    }
    if (ready) void scheduleReconcile();
  };

  const handleError = (key: SourceKey, error: string) => {
    const gate = gates.get(key)!;
    if (!gate.settled) {
      gate.settled = true;
      gate.reject(new Error(error));
      return;
    }
    console.error(`Failed to read ${key} for search indexing:`, error);
  };

  const register = async <Row>(
    key: SourceKey,
    sql: string,
    mapRows: (rows: Row[]) => SearchDocument[],
  ) => {
    const unsubscribe = await liveQueryClient.subscribe<Row>(sql, [], {
      onData: (rows) => handleRows(key, rows, mapRows),
      onError: (error) => handleError(key, error),
    });
    if (stopped) {
      await unsubscribe();
      return;
    }
    unsubscribers.push(unsubscribe);
  };

  const stop = async (): Promise<void> => {
    if (stopped) return;
    stopped = true;
    cancelInitialWait();
    const activeUnsubscribers = unsubscribers.splice(0);
    await Promise.allSettled(
      activeUnsubscribers.map((unsubscribe) => unsubscribe()),
    );
    await reconcileQueue;
  };

  const start = async (): Promise<void> => {
    try {
      await register<SessionSearchSqlRow>(
        "sessions",
        SESSION_SEARCH_SQL,
        mapSessionRows,
      );
      if (stopped) return;
      await register<HumanSearchSqlRow>(
        "humans",
        HUMAN_SEARCH_SQL,
        mapHumanRows,
      );
      if (stopped) return;
      await register<OrganizationSearchSqlRow>(
        "organizations",
        ORGANIZATION_SEARCH_SQL,
        mapOrganizationRows,
      );
      if (stopped) return;

      const initialized = await Promise.race([
        initialSnapshots.then(() => true),
        cancelled.then(() => false),
      ]);
      if (!initialized || stopped) return;

      const initialDocuments = collectDocuments(snapshots);
      expectTantivySuccess("clear search index", await tantivy.reindex(null));
      if (stopped) return;
      if (initialDocuments.length > 0) {
        expectTantivySuccess(
          "index SQLite records",
          await tantivy.updateDocuments(initialDocuments, null),
        );
      }

      indexedDocuments = toDocumentMap(initialDocuments);
      ready = true;
      await scheduleReconcile();
    } catch (error) {
      await stop();
      throw error;
    }
  };

  return { start, stop };
}

function mapSessionRows(rows: SessionSearchSqlRow[]): SearchDocument[] {
  return rows.map((row) => {
    const enhancedNotesContent = mergeContent(
      parseStringArray(row.enhanced_notes_json).map(extractPlainText),
    );
    const transcript = mergeContent(
      parseStringArray(row.transcripts_json).map(flattenTranscript),
    );

    return {
      id: row.id,
      doc_type: "session",
      language: null,
      title: toTrimmedString(row.title) || "Untitled",
      content: createSessionSearchableContent({
        raw_md: row.raw_body,
        enhanced_notes_content: enhancedNotesContent,
        transcript,
      }),
      created_at: getSessionSearchTimestamp(row),
      facets: [],
    };
  });
}

function mapHumanRows(rows: HumanSearchSqlRow[]): SearchDocument[] {
  return rows.map((row) => ({
    id: row.id,
    doc_type: "human",
    language: null,
    title: toTrimmedString(row.name) || "Unknown",
    content: createHumanSearchableContent(row),
    created_at: toEpochMs(row.created_at),
    facets: [],
  }));
}

function mapOrganizationRows(
  rows: OrganizationSearchSqlRow[],
): SearchDocument[] {
  return rows.map((row) => ({
    id: row.id,
    doc_type: "organization",
    language: null,
    title: toTrimmedString(row.name) || "Unknown Organization",
    content: "",
    created_at: toEpochMs(row.created_at),
    facets: [],
  }));
}

async function reconcileDocuments(
  current: Map<string, SearchDocument>,
  nextDocuments: SearchDocument[],
): Promise<Map<string, SearchDocument>> {
  const next = toDocumentMap(nextDocuments);
  const changed = nextDocuments.filter(
    (document) => !documentsEqual(current.get(document.id), document),
  );
  const removedIds = Array.from(current.keys()).filter((id) => !next.has(id));

  if (changed.length > 0) {
    expectTantivySuccess(
      "update search documents",
      await tantivy.updateDocuments(changed, null),
    );
  }
  for (const id of removedIds) {
    expectTantivySuccess(
      `remove search document ${id}`,
      await tantivy.removeDocument(id, null),
    );
  }

  return next;
}

function collectDocuments(
  snapshots: Map<SourceKey, SearchDocument[]>,
): SearchDocument[] {
  return ["sessions", "humans", "organizations"].flatMap(
    (key) => snapshots.get(key as SourceKey) ?? [],
  );
}

function toDocumentMap(
  documents: SearchDocument[],
): Map<string, SearchDocument> {
  return new Map(documents.map((document) => [document.id, document]));
}

function documentsEqual(
  left: SearchDocument | undefined,
  right: SearchDocument,
): boolean {
  const leftFacets = left?.facets ?? [];
  const rightFacets = right.facets ?? [];
  return (
    left?.doc_type === right.doc_type &&
    left.language === right.language &&
    left.title === right.title &&
    left.content === right.content &&
    left.created_at === right.created_at &&
    leftFacets.length === rightFacets.length &&
    leftFacets.every((facet, index) => facet === rightFacets[index])
  );
}

function parseStringArray(value: string): string[] {
  try {
    const parsed = JSON.parse(value) as unknown;
    return Array.isArray(parsed)
      ? parsed.filter((item): item is string => typeof item === "string")
      : [];
  } catch {
    return [];
  }
}

function createInitialGate(): InitialGate {
  let resolve = () => {};
  let reject = (_error: Error) => {};
  const promise = new Promise<void>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject, settled: false };
}

function expectTantivySuccess(
  operation: string,
  result: { status: "ok"; data: unknown } | { status: "error"; error: string },
): void {
  if (result.status === "error") {
    throw new Error(`Failed to ${operation}: ${result.error}`);
  }
}
