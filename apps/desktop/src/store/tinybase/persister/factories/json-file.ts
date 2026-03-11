import { sep } from "@tauri-apps/api/path";
import { createCustomPersister } from "tinybase/persisters/with-schemas";
import type {
  PersistedChanges,
  Persists,
} from "tinybase/persisters/with-schemas";
import type { MergeableStore, OptionalSchemas } from "tinybase/with-schemas";

import {
  commands as fsSyncCommands,
  type JsonValue,
} from "@hypr/plugin-fs-sync";
import { commands as fs2Commands } from "@hypr/plugin-fs2";
import { events as notifyEvents } from "@hypr/plugin-notify";
import { commands as settingsCommands } from "@hypr/plugin-settings";
import { asTablesChanges, extractChangedTables } from "@hypr/tinybase-utils";

import { isFileNotFoundError } from "~/store/tinybase/persister/shared/fs";
import type { ChangedTables } from "~/store/tinybase/persister/shared/types";
import { StoreOrMergeableStore } from "~/store/tinybase/store/shared";

export type ListenMode = "notify" | "poll" | "both";

type ListenerHandle = {
  unlisten: (() => void) | null;
  interval: ReturnType<typeof setInterval> | null;
};

type TablesSchemaOf<S extends OptionalSchemas> = S extends [infer T, unknown]
  ? T
  : never;

export type JsonFieldMapping = Record<string, string>;

export function createJsonFilePersister<
  Schemas extends OptionalSchemas,
  TName extends keyof TablesSchemaOf<Schemas> & string,
>(
  store: MergeableStore<Schemas>,
  options: {
    tableName: TName;
    filename: string;
    label: string;
    listenMode?: ListenMode;
    pollIntervalMs?: number;
    jsonFields?: JsonFieldMapping;
  },
) {
  const {
    tableName,
    filename,
    label,
    listenMode = "poll",
    pollIntervalMs = 3000,
    jsonFields,
  } = options;

  return createCustomPersister(
    store,
    async () => loadContent(filename, tableName, label, jsonFields),
    async (_, changes) =>
      saveContent<Schemas, TName>(
        store,
        changes,
        tableName,
        filename,
        label,
        jsonFields,
      ),
    (listener) =>
      addListener(
        listener,
        filename,
        tableName,
        label,
        listenMode,
        pollIntervalMs,
        jsonFields,
      ),
    delListener,
    (error) => console.error(`[${label}]:`, error),
    StoreOrMergeableStore,
  );
}

async function loadContent(
  filename: string,
  tableName: string,
  label: string,
  jsonFields?: JsonFieldMapping,
) {
  let data = await loadTableData(filename, label);
  if (!data) return undefined;
  if (jsonFields) {
    data = transformForLoad(data, jsonFields);
  }
  // Return 3-tuple to use applyChanges() semantics (TinyBase checks content[2] === 1)
  return asTablesChanges({ [tableName]: data }) as any;
}

type TableId<S extends OptionalSchemas> = keyof TablesSchemaOf<S> & string;

async function saveContent<
  Schemas extends OptionalSchemas,
  TName extends TableId<Schemas>,
>(
  store: MergeableStore<Schemas>,
  changes:
    | PersistedChanges<Schemas, Persists.StoreOrMergeableStore>
    | undefined,
  tableName: TName,
  filename: string,
  label: string,
  jsonFields?: JsonFieldMapping,
) {
  if (changes) {
    const changedTables = extractChangedTables<Schemas>(changes);
    if (changedTables && !changedTables[tableName as keyof ChangedTables]) {
      return;
    }
  }

  try {
    const baseResult = await settingsCommands.vaultBase();
    if (baseResult.status === "error") {
      throw new Error(baseResult.error);
    }
    const base = baseResult.data;
    let data = (store.getTable(tableName) ?? {}) as Record<
      string,
      Record<string, unknown>
    >;
    if (jsonFields) {
      data = transformForSave(data, jsonFields);
    }
    const path = [base, filename].join(sep());
    const result = await fsSyncCommands.writeJsonBatch([
      [data as JsonValue, path],
    ]);
    if (result.status === "error") {
      throw new Error(result.error);
    }
  } catch (error) {
    console.error(`[${label}] save error:`, error);
  }
}

function addListener(
  listener: (content?: any, changes?: any) => void,
  filename: string,
  tableName: string,
  label: string,
  listenMode: ListenMode,
  pollIntervalMs: number,
  jsonFields?: JsonFieldMapping,
): ListenerHandle {
  const handle: ListenerHandle = { unlisten: null, interval: null };

  const onFileChange = async () => {
    let data = await loadTableData(filename, label);
    if (data) {
      if (jsonFields) {
        data = transformForLoad(data, jsonFields);
      }
      // Pass as changes (second param) with 3-tuple format for applyChanges() semantics
      listener(undefined, asTablesChanges({ [tableName]: data }) as any);
    }
  };

  if (listenMode === "notify" || listenMode === "both") {
    (async () => {
      const unlisten = await notifyEvents.fileChanged.listen((event) => {
        if (event.payload.path.endsWith(filename)) {
          onFileChange();
        }
      });
      handle.unlisten = unlisten;
    })().catch((error) => {
      console.error(`[${label}] Failed to setup notify listener:`, error);
    });
  }

  if (listenMode === "poll" || listenMode === "both") {
    handle.interval = setInterval(onFileChange, pollIntervalMs);
  }

  return handle;
}

function delListener(handle: ListenerHandle) {
  handle.unlisten?.();
  if (handle.interval) clearInterval(handle.interval);
}

type TableData = Record<string, Record<string, unknown>>;

function transformForSave(
  data: TableData,
  jsonFields: JsonFieldMapping,
): TableData {
  const result: TableData = {};
  for (const [rowId, row] of Object.entries(data)) {
    const newRow: Record<string, unknown> = { ...row };
    for (const [storageName, persistedName] of Object.entries(jsonFields)) {
      if (storageName in newRow) {
        const value = newRow[storageName];
        delete newRow[storageName];
        if (typeof value === "string" && value) {
          try {
            newRow[persistedName] = JSON.parse(value);
          } catch {}
        }
      }
    }
    result[rowId] = newRow;
  }
  return result;
}

function transformForLoad(
  data: TableData,
  jsonFields: JsonFieldMapping,
): TableData {
  const result: TableData = {};
  for (const [rowId, row] of Object.entries(data)) {
    const newRow: Record<string, unknown> = { ...row };
    for (const [storageName, persistedName] of Object.entries(jsonFields)) {
      if (persistedName in newRow) {
        const value = newRow[persistedName];
        delete newRow[persistedName];
        if (value !== undefined && value !== null) {
          newRow[storageName] = JSON.stringify(value);
        }
      }
    }
    result[rowId] = newRow;
  }
  return result;
}

async function loadTableData(
  filename: string,
  label: string,
): Promise<Record<string, Record<string, unknown>> | undefined> {
  const baseResult = await settingsCommands.vaultBase();
  if (baseResult.status === "error") {
    console.error(`[${label}] base error:`, baseResult.error);
    return undefined;
  }
  const base = baseResult.data;
  const path = [base, filename].join(sep());
  const result = await fs2Commands.readTextFile(path);

  if (result.status === "error") {
    if (!isFileNotFoundError(result.error)) {
      console.error(`[${label}] load error:`, result.error);
    }
    return undefined;
  }

  try {
    return JSON.parse(result.data);
  } catch (error) {
    console.error(`[${label}] JSON parse error:`, error);
    return undefined;
  }
}
