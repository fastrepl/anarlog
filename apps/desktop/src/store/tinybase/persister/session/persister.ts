import type { Schemas } from "@hypr/store";

import {
  getChangedSessionIds,
  getSessionSaveScope,
  parseSessionIdFromPath,
} from "./changes";
import {
  loadAllSessionData,
  type LoadedSessionData,
  loadSingleSession,
} from "./load/index";
import {
  buildNoteSaveOps,
  buildSessionSaveOps,
  buildTranscriptSaveOps,
} from "./save/index";

import { createMultiTableDirPersister } from "~/store/tinybase/persister/factories";
import type { Store } from "~/store/tinybase/store/main";

export function createSessionPersister(store: Store) {
  return createMultiTableDirPersister<Schemas, LoadedSessionData>(store, {
    label: "SessionPersister",
    dirName: "sessions",
    entityParser: parseSessionIdFromPath,
    tables: [
      { tableName: "sessions", isPrimary: true },
      { tableName: "mapping_session_participant", foreignKey: "session_id" },
      { tableName: "tags" },
      { tableName: "mapping_tag_session", foreignKey: "session_id" },
      { tableName: "transcripts", foreignKey: "session_id" },
      { tableName: "enhanced_notes", foreignKey: "session_id" },
    ],
    loadAll: loadAllSessionData,
    loadSingle: loadSingleSession,
    save: (store, tables, dataDir, changedTables) => {
      let changedSessionIds: Set<string> | undefined;
      const saveScope = getSessionSaveScope(changedTables);

      if (changedTables) {
        const changeResult = getChangedSessionIds(tables, changedTables);
        if (!changeResult) {
          return { operations: [] };
        }

        if (changeResult.hasUnresolvedDeletions) {
          changedSessionIds = undefined;
        } else {
          changedSessionIds = changeResult.changedSessionIds;
        }
      }

      const sessionOps = saveScope.session
        ? buildSessionSaveOps(store, tables, dataDir, changedSessionIds)
        : [];
      const transcriptOps = saveScope.transcript
        ? buildTranscriptSaveOps(tables, dataDir, changedSessionIds)
        : [];
      const noteOps = saveScope.note
        ? buildNoteSaveOps(store, tables, dataDir, changedSessionIds)
        : [];

      return {
        operations: [...sessionOps, ...transcriptOps, ...noteOps],
      };
    },
  });
}
