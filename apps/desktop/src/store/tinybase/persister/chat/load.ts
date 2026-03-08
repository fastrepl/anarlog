import { sep } from "@tauri-apps/api/path";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { commands as fs2Commands } from "@hypr/plugin-fs2";

import type { ChatJson, LoadedChatData } from "./types";

import {
  CHAT_MESSAGES_FILE,
  err,
  isDirectoryNotFoundError,
  isFileNotFoundError,
  type LoadResult,
  ok,
} from "~/store/tinybase/persister/shared";

export type { LoadedChatData } from "./types";

const LABEL = "ChatPersister";

export function chatJsonToData(json: ChatJson): LoadedChatData {
  const result: LoadedChatData = {
    chat_groups: {},
    chat_messages: {},
  };

  const { id: groupId, ...chatGroupData } = json.chat_group;
  result.chat_groups[groupId] = chatGroupData;

  for (const message of json.messages) {
    const { id: messageId, ...messageData } = message;
    result.chat_messages[messageId] = messageData;
  }

  return result;
}

export function mergeLoadedData(items: LoadedChatData[]): LoadedChatData {
  const result: LoadedChatData = {
    chat_groups: {},
    chat_messages: {},
  };

  for (const item of items) {
    Object.assign(result.chat_groups, item.chat_groups);
    Object.assign(result.chat_messages, item.chat_messages);
  }

  return result;
}

export function createEmptyLoadedChatData(): LoadedChatData {
  return {
    chat_groups: {},
    chat_messages: {},
  };
}

export async function loadAllChatGroups(
  dataDir: string,
): Promise<LoadResult<LoadedChatData>> {
  const chatsDir = [dataDir, "chats"].join(sep());

  const scanResult = await fsSyncCommands.scanAndRead(
    chatsDir,
    [CHAT_MESSAGES_FILE],
    false,
    null,
  );

  if (scanResult.status === "error") {
    if (isDirectoryNotFoundError(scanResult.error)) {
      return ok(createEmptyLoadedChatData());
    }
    console.error(`[${LABEL}] scan error:`, scanResult.error);
    return err(scanResult.error);
  }

  const { files } = scanResult.data;
  const items: LoadedChatData[] = [];

  for (const [, content] of Object.entries(files)) {
    if (!content) continue;
    try {
      const json = JSON.parse(content) as ChatJson;
      items.push(chatJsonToData(json));
    } catch (error) {
      console.error(`[${LABEL}] Failed to parse chat JSON:`, error);
    }
  }

  return ok(mergeLoadedData(items));
}

export async function loadSingleChatGroup(
  dataDir: string,
  groupId: string,
): Promise<LoadResult<LoadedChatData>> {
  const filePath = [dataDir, "chats", groupId, CHAT_MESSAGES_FILE].join(sep());

  const result = await fs2Commands.readTextFile(filePath);
  if (result.status === "error") {
    if (isFileNotFoundError(result.error)) {
      return ok(createEmptyLoadedChatData());
    }
    console.error(
      `[${LABEL}] Failed to load chat group ${groupId}:`,
      result.error,
    );
    return err(result.error);
  }

  try {
    const json = JSON.parse(result.data) as ChatJson;
    return ok(chatJsonToData(json));
  } catch (error) {
    console.error(
      `[${LABEL}] Failed to parse chat JSON for ${groupId}:`,
      error,
    );
    return err(String(error));
  }
}
