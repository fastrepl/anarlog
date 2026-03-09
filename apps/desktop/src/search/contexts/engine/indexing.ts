import { type SearchDocument, commands as tantivy } from "@hypr/plugin-tantivy";

import {
  createHumanSearchableContent,
  createSessionSearchableContent,
} from "./content";
import {
  collectCells,
  collectEnhancedNotesContent,
  toEpochMs,
  toTrimmedString,
} from "./utils";

import { type Store as MainStore } from "~/store/tinybase/store/main";

export async function indexSessions(store: MainStore): Promise<void> {
  const fields = [
    "user_id",
    "created_at",
    "folder_id",
    "event_json",
    "title",
    "raw_md",
    "transcript",
  ];

  const documents: SearchDocument[] = [];

  store.forEachRow("sessions", (rowId: string, _forEachCell) => {
    const row = collectCells(store, "sessions", rowId, fields);
    row.enhanced_notes_content = collectEnhancedNotesContent(store, rowId);
    const title = toTrimmedString(row.title) || "Untitled";

    documents.push({
      id: rowId,
      doc_type: "session",
      language: null,
      title,
      content: createSessionSearchableContent(row),
      created_at: toEpochMs(row.created_at),
      facets: [],
    });
  });

  if (documents.length > 0) {
    await tantivy.updateDocuments(documents, null);
  }
}

export async function indexHumans(store: MainStore): Promise<void> {
  const fields = [
    "name",
    "email",
    "org_id",
    "job_title",
    "linkedin_username",
    "created_at",
    "memo",
  ];

  const documents: SearchDocument[] = [];

  store.forEachRow("humans", (rowId: string, _forEachCell) => {
    const row = collectCells(store, "humans", rowId, fields);
    const title = toTrimmedString(row.name) || "Unknown";

    documents.push({
      id: rowId,
      doc_type: "human",
      language: null,
      title,
      content: createHumanSearchableContent(row),
      created_at: toEpochMs(row.created_at),
      facets: [],
    });
  });

  if (documents.length > 0) {
    await tantivy.updateDocuments(documents, null);
  }
}

export async function indexOrganizations(store: MainStore): Promise<void> {
  const fields = ["name", "created_at"];

  const documents: SearchDocument[] = [];

  store.forEachRow("organizations", (rowId: string, _forEachCell) => {
    const row = collectCells(store, "organizations", rowId, fields);
    const title = toTrimmedString(row.name) || "Unknown Organization";

    documents.push({
      id: rowId,
      doc_type: "organization",
      language: null,
      title,
      content: "",
      created_at: toEpochMs(row.created_at),
      facets: [],
    });
  });

  if (documents.length > 0) {
    await tantivy.updateDocuments(documents, null);
  }
}
