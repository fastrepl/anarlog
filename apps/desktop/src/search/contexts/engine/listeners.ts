import { RowListener } from "tinybase/with-schemas";

import { commands as tantivy } from "@hypr/plugin-tantivy";

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

import { Schemas } from "~/store/tinybase/store/main";
import { type Store as MainStore } from "~/store/tinybase/store/main";

export function createSessionListener(): RowListener<
  Schemas,
  "sessions",
  null,
  MainStore
> {
  return (store, _, rowId) => {
    try {
      const rowExists = store.getRow("sessions", rowId);

      if (!rowExists) {
        void tantivy.removeDocument(rowId, null);
      } else {
        const fields = [
          "user_id",
          "created_at",
          "title",
          "raw_md",
          "transcript",
        ];
        const row = collectCells(store, "sessions", rowId, fields);
        row.enhanced_notes_content = collectEnhancedNotesContent(store, rowId);
        const title = toTrimmedString(row.title) || "Untitled";

        void tantivy.updateDocument(
          {
            id: rowId,
            doc_type: "session",
            language: null,
            title,
            content: createSessionSearchableContent(row),
            created_at: toEpochMs(row.created_at),
            facets: [],
          },
          null,
        );
      }
    } catch (error) {
      console.error("Failed to update session in search index:", error);
    }
  };
}

export function createHumanListener(): RowListener<
  Schemas,
  "humans",
  null,
  MainStore
> {
  return (store, _, rowId) => {
    try {
      const rowExists = store.getRow("humans", rowId);

      if (!rowExists) {
        void tantivy.removeDocument(rowId, null);
      } else {
        const fields = ["name", "email", "created_at"];
        const row = collectCells(store, "humans", rowId, fields);
        const title = toTrimmedString(row.name) || "Unknown";

        void tantivy.updateDocument(
          {
            id: rowId,
            doc_type: "human",
            language: null,
            title,
            content: createHumanSearchableContent(row),
            created_at: toEpochMs(row.created_at),
            facets: [],
          },
          null,
        );
      }
    } catch (error) {
      console.error("Failed to update human in search index:", error);
    }
  };
}

export function createOrganizationListener(): RowListener<
  Schemas,
  "organizations",
  null,
  MainStore
> {
  return (store, _, rowId) => {
    try {
      const rowExists = store.getRow("organizations", rowId);

      if (!rowExists) {
        void tantivy.removeDocument(rowId, null);
      } else {
        const fields = ["name", "created_at"];
        const row = collectCells(store, "organizations", rowId, fields);
        const title = toTrimmedString(row.name) || "Unknown Organization";

        void tantivy.updateDocument(
          {
            id: rowId,
            doc_type: "organization",
            language: null,
            title,
            content: "",
            created_at: toEpochMs(row.created_at),
            facets: [],
          },
          null,
        );
      }
    } catch (error) {
      console.error("Failed to update organization in search index:", error);
    }
  };
}
