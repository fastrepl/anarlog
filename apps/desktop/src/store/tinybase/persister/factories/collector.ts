import { createCustomPersister } from "tinybase/persisters/with-schemas";
import type {
  PersistedChanges,
  PersistedContent,
  Persists,
} from "tinybase/persisters/with-schemas";
import type {
  Content,
  MergeableStore,
  OptionalSchemas,
} from "tinybase/with-schemas";

import {
  commands as fsSyncCommands,
  type ParsedDocument,
} from "@hypr/plugin-fs-sync";
import { commands as fs2Commands } from "@hypr/plugin-fs2";
import { extractChangedTables } from "@hypr/tinybase-utils";

import {
  createFileListener,
  type NotifyListenerHandle,
} from "~/store/tinybase/persister/shared/listener";
import { getDataDir } from "~/store/tinybase/persister/shared/paths";
import {
  type ChangedTables,
  type JsonValue,
  type SaveResult,
  type TablesContent,
} from "~/store/tinybase/persister/shared/types";
import { StoreOrMergeableStore } from "~/store/tinybase/store/shared";

const CLEANUP_SAFEGUARD_MIN_DISK_COUNT = 5;
const CLEANUP_SAFEGUARD_MIN_KEEP_RATIO = 0.5;

type LoadSingleFn<Schemas extends OptionalSchemas> = (
  entityId: string,
) => Promise<
  PersistedChanges<Schemas, Persists.StoreOrMergeableStore> | undefined
>;

type OrphanCleanupDirs = {
  type: "dirs";
  subdir: string;
  markerFile: string;
  keepIds: string[];
};

type OrphanCleanupFiles = {
  type: "files";
  subdir: string;
  extension: string;
  keepIds: string[];
};

type OrphanCleanupFilesRecursive = {
  type: "filesRecursive";
  subdir: string;
  markerFile: string;
  extension: string;
  keepIds: string[];
};

export type OrphanCleanupConfig =
  | OrphanCleanupDirs
  | OrphanCleanupFiles
  | OrphanCleanupFilesRecursive;

type BaseCollectorOptions<Schemas extends OptionalSchemas> = {
  label: string;
  save: (
    store: MergeableStore<Schemas>,
    tables: TablesContent,
    dataDir: string,
    changedTables?: ChangedTables,
  ) => SaveResult;
  load?: () => Promise<Content<Schemas> | undefined>;
  cleanup?: (tables: TablesContent) => OrphanCleanupConfig[];
  watchPaths?: string[];
  watchIntervalMs?: number;
};

type CollectorOptionsWithEntityLoading<Schemas extends OptionalSchemas> =
  BaseCollectorOptions<Schemas> & {
    loadSingle: LoadSingleFn<Schemas>;
    entityParser: (path: string) => string | null;
  };

type CollectorOptionsWithoutEntityLoading<Schemas extends OptionalSchemas> =
  BaseCollectorOptions<Schemas> & {
    loadSingle?: never;
    entityParser?: never;
  };

export type CollectorOptions<Schemas extends OptionalSchemas> =
  | CollectorOptionsWithEntityLoading<Schemas>
  | CollectorOptionsWithoutEntityLoading<Schemas>;

export function createCollectorPersister<Schemas extends OptionalSchemas>(
  store: MergeableStore<Schemas>,
  options: CollectorOptions<Schemas>,
) {
  const loadFn = options.load ?? (async () => undefined);

  const pathMatcher = (path: string) =>
    options.watchPaths?.some((p) => path.startsWith(p)) ?? false;

  const useEntityMode =
    options.watchPaths && options.loadSingle && options.entityParser;

  const fileListener = options.watchPaths
    ? useEntityMode
      ? createFileListener({
          mode: "entity",
          pathMatcher,
          entityParser: options.entityParser!,
        })
      : createFileListener({
          mode: "simple",
          pathMatcher,
          fallbackIntervalMs: options.watchIntervalMs ?? 30000,
        })
    : null;

  const saveFn = async (
    _getContent: () => PersistedContent<
      Schemas,
      Persists.StoreOrMergeableStore
    >,
    changes?: PersistedChanges<Schemas, Persists.StoreOrMergeableStore>,
  ) => {
    const changedTables = extractChangedTables<Schemas>(changes);

    try {
      const dataDir = await getDataDir();
      const tables = store.getTables();
      const result = options.save(
        store,
        tables,
        dataDir,
        changedTables ?? undefined,
      );
      const { operations } = result;

      if (operations.length > 0) {
        const jsonItems: Array<[JsonValue, string]> = [];
        const documentItems: Array<[ParsedDocument, string]> = [];
        const deletePaths: string[] = [];

        for (const op of operations) {
          if (op.type === "write-json") {
            jsonItems.push([op.content as JsonValue, op.path]);
          } else if (op.type === "write-document-batch") {
            documentItems.push(...op.items);
          } else if (op.type === "delete") {
            deletePaths.push(...op.paths);
          }
        }

        await writeJsonBatch(jsonItems, options.label);
        await writeDocumentBatch(documentItems, options.label);
        await deleteFiles(deletePaths, options.label);
      }

      if (options.cleanup) {
        const cleanupConfigs = options.cleanup(tables ?? {});
        await runOrphanCleanup(cleanupConfigs, options.label);
      }
    } catch (error) {
      console.error(`[${options.label}] save error:`, error);
    }
  };

  return createCustomPersister(
    store,
    loadFn,
    saveFn,
    (listener) => {
      if (!fileListener) return null;

      if (fileListener.mode === "entity") {
        return fileListener.addListener(async ({ entityId }) => {
          try {
            const changes = await options.loadSingle!(entityId);
            if (changes) {
              listener(undefined, changes);
            }
          } catch (error) {
            console.error(
              `[${options.label}] loadSingle error for ${entityId}:`,
              error,
            );
            listener();
          }
        });
      }

      return fileListener.addListener(() => listener());
    },
    (handle: NotifyListenerHandle | null) => {
      if (handle && fileListener) {
        fileListener.delListener(handle);
      }
    },
    (error) => console.error(`[${options.label}]:`, error),
    StoreOrMergeableStore,
  );
}

async function writeJsonBatch(
  items: Array<[JsonValue, string]>,
  label: string,
): Promise<void> {
  if (items.length === 0) return;

  const result = await fsSyncCommands.writeJsonBatch(items);
  if (result.status === "error") {
    console.error(`[${label}] Failed to export json batch:`, result.error);
  }
}

async function writeDocumentBatch(
  items: Array<[ParsedDocument, string]>,
  label: string,
): Promise<void> {
  if (items.length === 0) return;

  const result = await fsSyncCommands.writeDocumentBatch(items);
  if (result.status === "error") {
    console.error(`[${label}] Failed to write document batch:`, result.error);
  }
}

async function deleteFiles(paths: string[], label: string): Promise<void> {
  if (paths.length === 0) return;

  for (const path of paths) {
    const result = await fs2Commands.remove(path);
    if (result.status === "error") {
      const errorStr = result.error;
      if (!errorStr.includes("No such file") && !errorStr.includes("ENOENT")) {
        console.error(`[${label}] Failed to delete file ${path}:`, errorStr);
      }
    }
  }
}

async function countItemsOnDisk(config: OrphanCleanupConfig): Promise<number> {
  try {
    const dataDir = await getDataDir();
    const subdir = [dataDir, config.subdir].join("/");

    if (config.type === "dirs") {
      const result = await fsSyncCommands.scanAndRead(
        subdir,
        [config.markerFile],
        true,
        null,
      );
      if (result.status === "ok") {
        return result.data.dirs.length;
      }
    } else if (config.type === "files") {
      const result = await fsSyncCommands.scanAndRead(
        subdir,
        [`*.${config.extension}`],
        false,
        null,
      );
      if (result.status === "ok") {
        return Object.keys(result.data.files).length;
      }
    } else if (config.type === "filesRecursive") {
      const result = await fsSyncCommands.scanAndRead(
        subdir,
        [`*.${config.extension}`],
        true,
        null,
      );
      if (result.status === "ok") {
        return Object.keys(result.data.files).length;
      }
    }
  } catch {
    // Ignore counting errors - we'll proceed with cleanup
  }
  return 0;
}

async function runOrphanCleanup(
  configs: OrphanCleanupConfig[],
  label: string,
): Promise<void> {
  for (const config of configs) {
    if (config.keepIds.length === 0) {
      continue;
    }

    const diskCount = await countItemsOnDisk(config);
    const keepRatio = config.keepIds.length / Math.max(diskCount, 1);

    if (
      diskCount > CLEANUP_SAFEGUARD_MIN_DISK_COUNT &&
      keepRatio < CLEANUP_SAFEGUARD_MIN_KEEP_RATIO
    ) {
      console.warn(
        `[${label}] Skipping ${config.type} cleanup: keeping ${config.keepIds.length}/${diskCount} ` +
          `(${(keepRatio * 100).toFixed(0)}%) - possible load failure`,
      );
      continue;
    }

    try {
      if (config.type === "dirs") {
        await fsSyncCommands.cleanupOrphan(
          {
            type: "dirs",
            subdir: config.subdir,
            marker_file: config.markerFile,
          },
          config.keepIds,
        );
      } else if (config.type === "files") {
        await fsSyncCommands.cleanupOrphan(
          { type: "files", subdir: config.subdir, extension: config.extension },
          config.keepIds,
        );
      } else if (config.type === "filesRecursive") {
        await fsSyncCommands.cleanupOrphan(
          {
            type: "filesRecursive",
            subdir: config.subdir,
            marker_file: config.markerFile,
            extension: config.extension,
          },
          config.keepIds,
        );
      }
    } catch (error) {
      console.error(`[${label}] Cleanup error for ${config.type}:`, error);
    }
  }
}
