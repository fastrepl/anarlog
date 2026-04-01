import type { MergeableStore, OptionalSchemas } from "tinybase/with-schemas";

import { toContent, toPersistedChanges } from "@hypr/tinybase-utils";

import {
  createCollectorPersister,
  type OrphanCleanupConfig,
} from "./collector";

import {
  createDeletionMarker,
  type DeletionMarkerStore,
  type TableConfigEntry,
} from "~/store/tinybase/persister/shared/deletion-marker";
import type { LoadResult } from "~/store/tinybase/persister/shared/load-result";
import { getDataDir } from "~/store/tinybase/persister/shared/paths";
import type {
  ChangedTables,
  SaveResult,
  TablesContent,
} from "~/store/tinybase/persister/shared/types";

type Table = Record<string, Record<string, unknown>>;

export type MultiTableDirConfig<
  Schemas extends OptionalSchemas,
  TLoadedData extends Record<string, Table>,
> = {
  label: string;
  dirName: string;
  entityParser: (path: string) => string | null;
  tables: TableConfigEntry<Schemas, TLoadedData>[];
  cleanup: (tables: TablesContent) => OrphanCleanupConfig[];
  loadAll: (dataDir: string) => Promise<LoadResult<TLoadedData>>;
  loadSingle: (
    dataDir: string,
    entityId: string,
  ) => Promise<LoadResult<TLoadedData>>;
  save: (
    store: MergeableStore<Schemas>,
    tables: TablesContent,
    dataDir: string,
    changedTables?: ChangedTables,
  ) => SaveResult;
};

function hasChanges<TLoadedData extends Record<string, Table>>(
  result: { [K in keyof TLoadedData]: Record<string, unknown> },
  tableNames: (keyof TLoadedData)[],
): boolean {
  return tableNames.some((name) => Object.keys(result[name] ?? {}).length > 0);
}

export function createMultiTableDirPersister<
  Schemas extends OptionalSchemas,
  TLoadedData extends Record<string, Table>,
>(
  store: MergeableStore<Schemas>,
  config: MultiTableDirConfig<Schemas, TLoadedData>,
): ReturnType<typeof createCollectorPersister<Schemas>> {
  const {
    label,
    dirName,
    entityParser,
    tables,
    cleanup,
    loadAll,
    loadSingle,
    save,
  } = config;

  const deletionMarker = createDeletionMarker<TLoadedData>(
    store as DeletionMarkerStore,
    tables,
  );
  const tableNames = tables.map((t) => t.tableName);

  return createCollectorPersister(store, {
    label,
    watchPaths: [`${dirName}/`],
    cleanup,
    entityParser,
    loadSingle: async (entityId: string) => {
      try {
        const dataDir = await getDataDir();
        const loadResult = await loadSingle(dataDir, entityId);

        if (loadResult.status === "error") {
          console.error(
            `[${label}] loadSingle error for ${entityId}:`,
            loadResult.error,
          );
          return undefined;
        }

        const result = deletionMarker.markForEntity(loadResult.data, entityId);

        if (!hasChanges(result, tableNames)) {
          return undefined;
        }

        return toPersistedChanges<Schemas>(result);
      } catch (error) {
        console.error(`[${label}] loadSingle error for ${entityId}:`, error);
        return undefined;
      }
    },
    save,
    load: async () => {
      try {
        const dataDir = await getDataDir();
        const loadResult = await loadAll(dataDir);

        if (loadResult.status === "error") {
          console.error(`[${label}] load error:`, loadResult.error);
          return undefined;
        }

        const result = deletionMarker.markAll(loadResult.data);

        if (!hasChanges(result, tableNames)) {
          return undefined;
        }

        return toContent<Schemas>(result);
      } catch (error) {
        console.error(`[${label}] load error:`, error);
        return undefined;
      }
    },
  });
}
