import { sep } from "@tauri-apps/api/path";

import type { ChatGroupData, ChatJson, ChatMessageWithId } from "./types";

import {
  buildChatPath,
  CHAT_MESSAGES_FILE,
  iterateTableRows,
  type TablesContent,
  type WriteOperation,
} from "~/store/tinybase/persister/shared";

export function tablesToChatJsonList(tables: TablesContent): ChatJson[] {
  const chatGroups = iterateTableRows(tables, "chat_groups");
  const chatMessages = iterateTableRows(tables, "chat_messages");

  const chatGroupMap = new Map<string, ChatGroupData>();
  for (const group of chatGroups) {
    chatGroupMap.set(group.id, group as ChatGroupData);
  }

  const messagesByChatGroup = new Map<
    string,
    { chatGroup: ChatGroupData; messages: ChatMessageWithId[] }
  >();

  for (const message of chatMessages) {
    const chatGroupId = message.chat_group_id;
    if (!chatGroupId) continue;

    const chatGroup = chatGroupMap.get(chatGroupId);
    if (!chatGroup) continue;

    const existing = messagesByChatGroup.get(chatGroupId);
    if (existing) {
      existing.messages.push(message as ChatMessageWithId);
    } else {
      messagesByChatGroup.set(chatGroupId, {
        chatGroup,
        messages: [message as ChatMessageWithId],
      });
    }
  }

  const result: ChatJson[] = [];
  for (const { chatGroup, messages } of messagesByChatGroup.values()) {
    result.push({
      chat_group: chatGroup,
      messages: messages.sort(
        (a, b) =>
          new Date(a.created_at || 0).getTime() -
          new Date(b.created_at || 0).getTime(),
      ),
    });
  }

  return result;
}

export function buildChatSaveOps(
  tables: TablesContent,
  dataDir: string,
  changedGroupIds?: Set<string>,
): WriteOperation[] {
  const operations: WriteOperation[] = [];

  const chatJsonList = tablesToChatJsonList(tables);
  const allGroupIds = new Set(Object.keys(tables.chat_groups ?? {}));
  const groupsWithMessages = new Set(chatJsonList.map((c) => c.chat_group.id));

  if (changedGroupIds) {
    const deletedIds: string[] = [];

    for (const id of changedGroupIds) {
      if (groupsWithMessages.has(id)) {
        const chatJson = chatJsonList.find((c) => c.chat_group.id === id)!;
        const chatDir = buildChatPath(dataDir, id);
        operations.push({
          type: "write-json",
          path: [chatDir, CHAT_MESSAGES_FILE].join(sep()),
          content: chatJson,
        });
      } else if (allGroupIds.has(id)) {
        const chatGroup = tables.chat_groups![id];
        const chatDir = buildChatPath(dataDir, id);
        operations.push({
          type: "write-json",
          path: [chatDir, CHAT_MESSAGES_FILE].join(sep()),
          content: { chat_group: { id, ...chatGroup }, messages: [] },
        });
      } else {
        deletedIds.push(id);
      }
    }

    if (deletedIds.length > 0) {
      const deleteOps: WriteOperation = {
        type: "delete",
        paths: deletedIds.map((id) =>
          [buildChatPath(dataDir, id), CHAT_MESSAGES_FILE].join(sep()),
        ),
      };
      operations.push(deleteOps);
    }

    return operations;
  }

  for (const chatJson of chatJsonList) {
    const chatGroupId = chatJson.chat_group.id;
    const chatDir = buildChatPath(dataDir, chatGroupId);

    operations.push({
      type: "write-json",
      path: [chatDir, CHAT_MESSAGES_FILE].join(sep()),
      content: chatJson,
    });
  }

  for (const id of allGroupIds) {
    if (!groupsWithMessages.has(id)) {
      const chatGroup = tables.chat_groups![id];
      const chatDir = buildChatPath(dataDir, id);
      operations.push({
        type: "write-json",
        path: [chatDir, CHAT_MESSAGES_FILE].join(sep()),
        content: { chat_group: { id, ...chatGroup }, messages: [] },
      });
    }
  }

  return operations;
}
