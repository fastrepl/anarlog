import type { Schemas } from "@hypr/store";

import { getChangedChatGroupIds, parseChatGroupIdFromPath } from "./changes";
import {
  loadAllChatGroups,
  type LoadedChatData,
  loadSingleChatGroup,
} from "./load";
import { buildChatSaveOps } from "./save";

import { createMultiTableDirPersister } from "~/store/tinybase/persister/factories";
import { CHAT_MESSAGES_FILE } from "~/store/tinybase/persister/shared";
import type { Store } from "~/store/tinybase/store/main";

export function createChatPersister(store: Store) {
  return createMultiTableDirPersister<Schemas, LoadedChatData>(store, {
    label: "ChatPersister",
    dirName: "chats",
    entityParser: parseChatGroupIdFromPath,
    tables: [
      { tableName: "chat_groups", isPrimary: true },
      { tableName: "chat_messages", foreignKey: "chat_group_id" },
    ],
    cleanup: (tables) => [
      {
        type: "dirs",
        subdir: "chats",
        markerFile: CHAT_MESSAGES_FILE,
        keepIds: Object.keys(tables.chat_groups ?? {}),
      },
    ],
    loadAll: loadAllChatGroups,
    loadSingle: loadSingleChatGroup,
    save: (_store, tables, dataDir, changedTables) => {
      let changedGroupIds: Set<string> | undefined;

      if (changedTables) {
        const changeResult = getChangedChatGroupIds(tables, changedTables);
        if (!changeResult) {
          return { operations: [] };
        }

        if (changeResult.hasUnresolvedDeletions) {
          changedGroupIds = undefined;
        } else {
          changedGroupIds = changeResult.changedChatGroupIds;
        }
      }

      return {
        operations: buildChatSaveOps(tables, dataDir, changedGroupIds),
      };
    },
  });
}
