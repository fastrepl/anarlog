import { eq, sessionDocuments, sql } from "@hypr/db";
import { md2json } from "@hypr/editor/markdown";

import type { Store } from "./main";
import {
  type MainTableRow,
  SqliteTableShadow,
  type SqliteTableShadowConfig,
} from "./sqlite-table-shadow";

import { db } from "~/db";

type SqliteDocumentRow = {
  id: string;
  session_id: string;
  kind: string;
  template_id: string;
  title: string;
  body_format: string;
  body: string;
  source_hash: string;
  sort_order: number;
  created_by: string;
  created_at: string;
  updated_at: string;
  deleted_at: string | null;
};

const ENHANCED_NOTE_SHADOW_CONFIG: SqliteTableShadowConfig<
  "enhanced_notes",
  SqliteDocumentRow
> = {
  label: "SqliteEnhancedNoteShadow",
  tableId: "enhanced_notes",
  selectSql: `
    SELECT
      id,
      session_id,
      kind,
      template_id,
      title,
      body_format,
      body,
      source_hash,
      sort_order,
      created_by,
      created_at,
      updated_at,
      deleted_at
    FROM session_documents
    WHERE kind IN ('summary', 'template_output')
    ORDER BY session_id, sort_order, id
  `,
  fromSqlite: (row) =>
    normalizeEnhancedNoteRow({
      user_id: row.created_by,
      session_id: row.session_id,
      content: decodeEditorBody(row.body, row.body_format),
      template_id: row.template_id,
      position: row.sort_order,
      title: row.title,
    }),
  normalize: normalizeEnhancedNoteRow,
  isDeleted: (row) => row.deleted_at !== null,
  persist: persistEnhancedNoteChanges,
};

const KEY_FACTS_SHADOW_CONFIG: SqliteTableShadowConfig<
  "session_key_facts",
  SqliteDocumentRow
> = {
  label: "SqliteKeyFactsShadow",
  tableId: "session_key_facts",
  selectSql: `
    SELECT
      id,
      session_id,
      kind,
      template_id,
      title,
      body_format,
      body,
      source_hash,
      sort_order,
      created_by,
      created_at,
      updated_at,
      deleted_at
    FROM session_documents
    WHERE kind = 'key_facts'
    ORDER BY session_id, id
  `,
  fromSqlite: (row) =>
    normalizeKeyFactsRow({
      user_id: row.created_by,
      session_id: row.session_id,
      created_at: row.created_at,
      updated_at: row.updated_at,
      content: row.body,
      source_hash: row.source_hash,
    }),
  normalize: normalizeKeyFactsRow,
  isDeleted: (row) => row.deleted_at !== null,
  persist: persistKeyFactsChanges,
};

export function SqliteDocumentShadow({ store }: { store: Store }) {
  return (
    <>
      <SqliteTableShadow config={ENHANCED_NOTE_SHADOW_CONFIG} store={store} />
      <SqliteTableShadow config={KEY_FACTS_SHADOW_CONFIG} store={store} />
    </>
  );
}

export function normalizeEnhancedNoteRow(
  row: Partial<MainTableRow<"enhanced_notes">>,
): MainTableRow<"enhanced_notes"> {
  return {
    user_id: typeof row.user_id === "string" ? row.user_id : "",
    session_id: typeof row.session_id === "string" ? row.session_id : "",
    content: typeof row.content === "string" ? row.content : "",
    template_id: typeof row.template_id === "string" ? row.template_id : "",
    position: typeof row.position === "number" ? row.position : 0,
    title: typeof row.title === "string" ? row.title : "",
  };
}

export function normalizeKeyFactsRow(
  row: Partial<MainTableRow<"session_key_facts">>,
): MainTableRow<"session_key_facts"> {
  return {
    user_id: typeof row.user_id === "string" ? row.user_id : "",
    session_id: typeof row.session_id === "string" ? row.session_id : "",
    created_at: typeof row.created_at === "string" ? row.created_at : "",
    updated_at: typeof row.updated_at === "string" ? row.updated_at : "",
    content: typeof row.content === "string" ? row.content : "",
    source_hash: typeof row.source_hash === "string" ? row.source_hash : "",
  };
}

async function persistEnhancedNoteChanges(
  rows: Array<[string, MainTableRow<"enhanced_notes">]>,
  deletedIds: string[],
): Promise<void> {
  const now = new Date().toISOString();
  for (const chunk of chunks(rows, 40)) {
    await db
      .insert(sessionDocuments)
      .values(
        chunk.map(([id, value]) => {
          const row = normalizeEnhancedNoteRow(value);
          return {
            id,
            workspaceId: "",
            sessionId: row.session_id,
            kind: row.template_id ? "template_output" : "summary",
            templateId: row.template_id,
            title: row.title,
            bodyFormat: "prosemirror_json",
            body: row.content,
            sourceHash: "",
            generationMetadataJson: "{}",
            sortOrder: row.position,
            createdBy: row.user_id,
            updatedBy: row.user_id,
            createdAt: now,
            updatedAt: now,
            deletedAt: null,
          };
        }),
      )
      .onConflictDoUpdate({
        target: sessionDocuments.id,
        set: {
          sessionId: sql`excluded.session_id`,
          kind: sql`excluded.kind`,
          templateId: sql`excluded.template_id`,
          title: sql`excluded.title`,
          bodyFormat: "prosemirror_json",
          body: sql`excluded.body`,
          sortOrder: sql`excluded.sort_order`,
          updatedBy: sql`excluded.updated_by`,
          deletedAt: null,
          updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`,
        },
      });
  }

  await softDeleteDocuments(deletedIds);
}

async function persistKeyFactsChanges(
  rows: Array<[string, MainTableRow<"session_key_facts">]>,
  deletedIds: string[],
): Promise<void> {
  const now = new Date().toISOString();
  for (const chunk of chunks(rows, 40)) {
    await db
      .insert(sessionDocuments)
      .values(
        chunk.map(([id, value]) => {
          const row = normalizeKeyFactsRow(value);
          return {
            id,
            workspaceId: "",
            sessionId: row.session_id,
            kind: "key_facts",
            templateId: "",
            title: "Key facts",
            bodyFormat: "markdown",
            body: row.content,
            sourceHash: row.source_hash,
            generationMetadataJson: "{}",
            sortOrder: 0,
            createdBy: row.user_id,
            updatedBy: row.user_id,
            createdAt: row.created_at || now,
            updatedAt: row.updated_at || now,
            deletedAt: null,
          };
        }),
      )
      .onConflictDoUpdate({
        target: sessionDocuments.id,
        set: {
          sessionId: sql`excluded.session_id`,
          body: sql`excluded.body`,
          sourceHash: sql`excluded.source_hash`,
          updatedBy: sql`excluded.updated_by`,
          deletedAt: null,
          updatedAt: sql`excluded.updated_at`,
        },
      });
  }

  await softDeleteDocuments(deletedIds);
}

async function softDeleteDocuments(ids: string[]): Promise<void> {
  const deletedAt = new Date().toISOString();
  for (const id of ids) {
    await db
      .update(sessionDocuments)
      .set({ deletedAt, updatedAt: deletedAt })
      .where(eq(sessionDocuments.id, id));
  }
}

function decodeEditorBody(body: string, format: string): string {
  if (!body || format !== "markdown") return body;
  try {
    return JSON.stringify(md2json(body));
  } catch (error) {
    console.error("[SqliteDocumentShadow] failed to decode Markdown", error);
    return body;
  }
}

function chunks<T>(items: T[], size: number): T[][] {
  const result: T[][] = [];
  for (let index = 0; index < items.length; index += size) {
    result.push(items.slice(index, index + size));
  }
  return result;
}
