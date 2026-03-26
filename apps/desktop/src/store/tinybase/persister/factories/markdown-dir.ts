import type { MergeableStore, OptionalSchemas } from "tinybase/with-schemas";

import {
  commands as fsSyncCommands,
  type JsonValue,
  type ParsedDocument,
} from "@hypr/plugin-fs-sync";
import { commands as fs2Commands } from "@hypr/plugin-fs2";
import { toContent, toPersistedChanges } from "@hypr/tinybase-utils";

import { createCollectorPersister } from "./collector";

import {
  createDeletionMarker,
  type DeletionMarkerStore,
} from "~/store/tinybase/persister/shared/deletion-marker";
import {
  isDirectoryNotFoundError,
  isFileNotFoundError,
} from "~/store/tinybase/persister/shared/fs";
import {
  err,
  type LoadResult,
  ok,
} from "~/store/tinybase/persister/shared/load-result";
import {
  buildEntityFilePath,
  buildEntityPath,
  getDataDir,
} from "~/store/tinybase/persister/shared/paths";
import {
  type ChangedTables,
  type WriteOperation,
} from "~/store/tinybase/persister/shared/types";

export interface MarkdownDirPersisterConfig<
  TStorage extends Record<string, unknown>,
> {
  tableName: string;
  dirName: string;
  label: string;
  entityParser: (path: string) => string | null;
  toFrontmatter: (entity: TStorage) => {
    frontmatter: Record<string, JsonValue>;
    body: string;
  };
  fromFrontmatter: (
    frontmatter: Record<string, unknown>,
    body: string,
  ) => TStorage;
}

type LoadedData<TStorage extends Record<string, unknown>> = {
  [tableName: string]: Record<string, TStorage>;
};

async function loadMarkdownDir<TStorage extends Record<string, unknown>>(
  dataDir: string,
  config: MarkdownDirPersisterConfig<TStorage>,
): Promise<LoadResult<Record<string, TStorage>>> {
  const { dirName, fromFrontmatter } = config;
  const dir = buildEntityPath(dataDir, dirName);
  const result = await fsSyncCommands.readDocumentBatch(dir);

  if (result.status === "error") {
    if (isDirectoryNotFoundError(result.error)) {
      return ok({});
    }
    return err(result.error);
  }

  const entities: Record<string, TStorage> = {};
  for (const [id, doc] of Object.entries(result.data)) {
    if (doc) {
      entities[id] = fromFrontmatter(
        doc.frontmatter as Record<string, unknown>,
        doc.content.trim(),
      );
    }
  }
  return ok(entities);
}

function collectMarkdownWriteOps<TStorage extends Record<string, unknown>>(
  tableData: Record<string, TStorage>,
  dataDir: string,
  config: MarkdownDirPersisterConfig<TStorage>,
): WriteOperation[] {
  const { dirName, toFrontmatter } = config;
  const operations: WriteOperation[] = [];

  const documentItems: [ParsedDocument, string][] = [];

  for (const [entityId, entity] of Object.entries(tableData)) {
    const { frontmatter, body } = toFrontmatter(entity);
    const filePath = buildEntityFilePath(dataDir, dirName, entityId);

    documentItems.push([{ frontmatter, content: body }, filePath]);
  }

  if (documentItems.length > 0) {
    operations.push({
      type: "write-document-batch",
      items: documentItems,
    });
  }

  return operations;
}

async function loadSingleEntity<
  Schemas extends OptionalSchemas,
  TStorage extends Record<string, unknown>,
>(
  config: MarkdownDirPersisterConfig<TStorage>,
  entityId: string,
  deletionMarker: ReturnType<typeof createDeletionMarker<LoadedData<TStorage>>>,
) {
  const { tableName, dirName, fromFrontmatter } = config;
  const dataDir = await getDataDir();
  const filePath = buildEntityFilePath(dataDir, dirName, entityId);

  const readResult = await fs2Commands.readTextFile(filePath);
  if (readResult.status === "error") {
    if (isFileNotFoundError(readResult.error)) {
      const loaded = { [tableName]: {} } as LoadedData<TStorage>;
      const result = deletionMarker.markForEntity(loaded, entityId);

      if (Object.keys(result[tableName] ?? {}).length > 0) {
        return toPersistedChanges<Schemas>(result);
      }
    }
    return undefined;
  }

  const parseResult = await fsSyncCommands.deserialize(readResult.data);

  if (parseResult.status === "error") {
    return undefined;
  }

  const entity = fromFrontmatter(
    parseResult.data.frontmatter as Record<string, unknown>,
    parseResult.data.content.trim(),
  );

  return toPersistedChanges<Schemas>({
    [tableName]: { [entityId]: entity as Record<string, unknown> },
  });
}

export function createMarkdownDirPersister<
  Schemas extends OptionalSchemas,
  TStorage extends Record<string, unknown>,
>(
  store: MergeableStore<Schemas>,
  config: MarkdownDirPersisterConfig<TStorage>,
): ReturnType<typeof createCollectorPersister<Schemas>> {
  const { tableName, dirName, label, entityParser } = config;

  const deletionMarker = createDeletionMarker<LoadedData<TStorage>>(
    store as DeletionMarkerStore,
    [{ tableName, isPrimary: true }],
  );

  return createCollectorPersister(store, {
    label,
    watchPaths: [`${dirName}/`],
    cleanup: (tables) => [
      {
        type: "files",
        subdir: dirName,
        extension: "md",
        keepIds: Object.keys(
          (tables as Record<string, Record<string, unknown>>)[tableName] ?? {},
        ),
      },
    ],
    entityParser,
    loadSingle: (entityId: string) =>
      loadSingleEntity(config, entityId, deletionMarker),
    save: (_store, tables, dataDir, changedTables) => {
      const fullTableData =
        (tables as Record<string, Record<string, TStorage>>)[tableName] ?? {};

      if (changedTables) {
        const changedRows = changedTables[tableName as keyof ChangedTables];

        if (!changedRows) {
          return { operations: [] };
        }

        const changedIds = Object.keys(changedRows);
        const filteredTableData: Record<string, TStorage> = {};
        const deletedIds: string[] = [];

        for (const id of changedIds) {
          const row = fullTableData[id];
          if (row) {
            filteredTableData[id] = row;
          } else {
            deletedIds.push(id);
          }
        }

        const writeOps = collectMarkdownWriteOps(
          filteredTableData,
          dataDir,
          config,
        );

        const deleteOps: WriteOperation[] =
          deletedIds.length > 0
            ? [
                {
                  type: "delete",
                  paths: deletedIds.map((id) =>
                    buildEntityFilePath(dataDir, dirName, id),
                  ),
                },
              ]
            : [];

        return {
          operations: [...writeOps, ...deleteOps],
        };
      }

      return {
        operations: collectMarkdownWriteOps(fullTableData, dataDir, config),
      };
    },
    load: async () => {
      const dataDir = await getDataDir();
      const loadResult = await loadMarkdownDir(dataDir, config);

      if (loadResult.status === "error") {
        console.error(`[${label}] load error:`, loadResult.error);
        return undefined;
      }

      const loaded = { [tableName]: loadResult.data } as LoadedData<TStorage>;
      const result = deletionMarker.markAll(loaded);

      if (Object.keys(result[tableName] ?? {}).length === 0) {
        return undefined;
      }

      return toContent<Schemas>(result);
    },
  });
}
