import { createMergeableStore } from "tinybase/with-schemas";

import { isValidContent, md2json } from "@hypr/editor/markdown";
import { SCHEMA } from "@hypr/store";

import type { Store } from "./main";

export type ImportResult =
  | { status: "success"; rowsImported: number; valuesImported: number }
  | { status: "error"; error: string };

type ParsedImport = {
  tables: object | null;
  values: object | null;
};

const parseImportData = (data: unknown): ParsedImport => {
  if (!Array.isArray(data) || data.length !== 2) {
    throw new Error("Invalid import format: expected [tables, values] array");
  }

  const [tables, values] = data as [unknown, unknown];

  if (tables !== null && typeof tables !== "object") {
    throw new Error("Invalid import format: tables must be an object or null");
  }
  if (values !== null && typeof values !== "object") {
    throw new Error("Invalid import format: values must be an object or null");
  }

  return { tables: tables as object | null, values: values as object | null };
};

const countRows = (tables: object | null): number => {
  if (!tables) return 0;
  let count = 0;
  for (const tableData of Object.values(tables)) {
    if (
      tableData &&
      typeof tableData === "object" &&
      !Array.isArray(tableData)
    ) {
      const keys = Object.keys(tableData);
      count += keys.length;
    }
  }
  return count;
};

const convertMarkdownToTiptapJson = (content: string): string => {
  if (!content || !content.trim()) {
    return content;
  }

  try {
    const parsed = JSON.parse(content);
    if (isValidContent(parsed)) {
      return content;
    }
  } catch {
    // Not JSON - treat as markdown
  }

  const tiptapJson = md2json(content);
  return JSON.stringify(tiptapJson);
};

type Tables = Record<string, Record<string, Record<string, unknown>>>;

const transformImportedTables = (tables: Tables): Tables => {
  if (tables.sessions) {
    for (const session of Object.values(tables.sessions)) {
      if (typeof session.raw_md === "string") {
        session.raw_md = convertMarkdownToTiptapJson(session.raw_md);
      }
    }
  }

  if (tables.enhanced_notes) {
    for (const note of Object.values(tables.enhanced_notes)) {
      if (typeof note.content === "string") {
        note.content = convertMarkdownToTiptapJson(note.content);
      }
    }
  }

  return tables;
};

const mergeImportData = (
  store: Store,
  { tables, values }: ParsedImport,
): { rowsImported: number; valuesImported: number } => {
  const importStore = createMergeableStore()
    .setTablesSchema(SCHEMA.table)
    .setValuesSchema(SCHEMA.value) as Store;

  let rowsImported = 0;
  if (tables) {
    rowsImported = countRows(tables);
    const transformedTables = transformImportedTables(tables as Tables);
    importStore.setTables(
      transformedTables as Parameters<Store["setTables"]>[0],
    );
  }

  if (values) {
    importStore.setValues(values as Parameters<Store["setValues"]>[0]);
  }

  store.transaction(() => {
    store.merge(importStore);
  });

  return {
    rowsImported,
    valuesImported: values ? Object.keys(values).length : 0,
  };
};

export const importData = async (
  store: Store,
  data: unknown,
  onPersistComplete: () => Promise<void>,
): Promise<ImportResult> => {
  try {
    const parsed = parseImportData(data);
    const { rowsImported, valuesImported } = mergeImportData(store, parsed);

    await onPersistComplete();

    console.log(
      `[Importer] Successfully imported ${rowsImported} rows and ${valuesImported} values`,
    );

    return { status: "success", rowsImported, valuesImported };
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    console.error("[Importer] Import failed:", errorMessage);
    return { status: "error", error: errorMessage };
  }
};
