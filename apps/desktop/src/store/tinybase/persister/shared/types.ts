import type { ParsedDocument } from "@hypr/plugin-fs-sync";
import { SCHEMA } from "@hypr/store";

import type { Store } from "~/store/tinybase/store/main";

export type BatchItem<T> = [T, string];

export type TablesContent = Partial<ReturnType<Store["getTables"]>>;

export type WriteOperation =
  | { type: "write-json"; path: string; content: unknown }
  | { type: "write-document-batch"; items: Array<[ParsedDocument, string]> }
  | { type: "delete"; paths: string[] };

export type SaveResult = {
  operations: WriteOperation[];
};

type TableNames = keyof typeof SCHEMA.table;

export type ChangedTables = Partial<{
  [K in TableNames]: Record<string, unknown> | undefined;
}>;

export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };
