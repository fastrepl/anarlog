import {
  type ChangedTables,
  createFolderEntityParser,
  getChangedIds,
  type TablesContent,
} from "~/store/tinybase/persister/shared";

export type ChatChangeResult = {
  changedChatGroupIds: Set<string>;
  hasUnresolvedDeletions: boolean;
};

export const parseChatGroupIdFromPath = createFolderEntityParser("chats");

export function getChangedChatGroupIds(
  tables: TablesContent,
  changedTables: ChangedTables,
): ChatChangeResult | undefined {
  const result = getChangedIds(tables, changedTables, [
    { table: "chat_groups", extractId: (id) => id },
    {
      table: "chat_messages",
      extractId: (id, tables) => tables.chat_messages?.[id]?.chat_group_id,
    },
  ]);

  if (!result) {
    return undefined;
  }

  return {
    changedChatGroupIds: result.changedIds,
    hasUnresolvedDeletions: result.hasUnresolvedDeletions,
  };
}
