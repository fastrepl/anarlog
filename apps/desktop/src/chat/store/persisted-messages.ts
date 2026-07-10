import type { ChatMessageStatus } from "@hypr/store";

import { hasRenderableContent } from "~/chat/message-content";
import type { HyprUIMessage } from "~/chat/types";

export type ChatMessageRecord = {
  id: string;
  ownerUserId: string;
  createdAt: string;
  chatGroupId: string;
  role: string;
  content: string;
  metadataJson: string;
  partsJson: string;
  status: ChatMessageStatus;
};

export type ChatMessageSqlRow = {
  id: string;
  owner_user_id: string;
  created_at: string;
  chat_group_id: string;
  role: string;
  content: string;
  metadata_json: string;
  parts_json: string;
  status: string;
};

export type PersistedChatMessage = {
  id: string;
  record: ChatMessageRecord;
  status: ChatMessageStatus;
  message: HyprUIMessage;
};

function parseJson<T>(value: string | undefined, fallback: T): T {
  if (!value) {
    return fallback;
  }

  try {
    return JSON.parse(value) as T;
  } catch {
    return fallback;
  }
}

export function normalizeChatMessageStatus(status: unknown): ChatMessageStatus {
  if (
    status === "streaming" ||
    status === "ready" ||
    status === "error" ||
    status === "aborted"
  ) {
    return status;
  }

  return "ready";
}

function extractTextContent(parts: HyprUIMessage["parts"]) {
  return parts
    .filter((part): part is Extract<typeof part, { type: "text" }> => {
      return part.type === "text";
    })
    .map((part) => part.text)
    .join("");
}

function getCreatedAt(
  message: HyprUIMessage,
  existingRecord?: Partial<ChatMessageRecord>,
) {
  if (existingRecord?.createdAt) {
    return existingRecord.createdAt;
  }

  const createdAt = message.metadata?.createdAt;
  if (typeof createdAt === "number") {
    return new Date(createdAt).toISOString();
  }

  return new Date().toISOString();
}

export function buildPersistedChatMessage({
  message,
  chatGroupId,
  ownerUserId,
  status,
  content,
  existingRecord,
}: {
  message: HyprUIMessage;
  chatGroupId: string;
  ownerUserId: string;
  status: ChatMessageStatus;
  content?: string;
  existingRecord?: Partial<ChatMessageRecord>;
}): ChatMessageRecord {
  return {
    id: message.id,
    ownerUserId,
    createdAt: getCreatedAt(message, existingRecord),
    chatGroupId,
    role: message.role,
    content: content ?? extractTextContent(message.parts),
    metadataJson: JSON.stringify(message.metadata ?? {}),
    partsJson: JSON.stringify(message.parts),
    status,
  };
}

export function rowToPersistedChatMessage(
  row: ChatMessageSqlRow,
): PersistedChatMessage {
  const status = normalizeChatMessageStatus(row.status);
  const message: HyprUIMessage = {
    id: row.id,
    role: row.role as "user" | "assistant",
    parts: parseJson(row.parts_json, []),
    metadata: parseJson(row.metadata_json, {}),
  };

  return {
    id: row.id,
    status,
    message,
    record: {
      id: row.id,
      ownerUserId: row.owner_user_id,
      createdAt: row.created_at,
      chatGroupId: row.chat_group_id,
      role: row.role,
      content: row.content,
      metadataJson: row.metadata_json || "{}",
      partsJson: row.parts_json || "[]",
      status,
    },
  };
}

export function shouldHidePersistedMessage(message: PersistedChatMessage) {
  return (
    message.message.role === "assistant" &&
    !hasRenderableContent(message.message) &&
    !message.record.content.trim()
  );
}

export function shouldPersistFinishedMessage(message: HyprUIMessage): boolean {
  return message.role !== "assistant" || hasRenderableContent(message);
}

export function getVisibleChatMessages(
  messages: PersistedChatMessage[],
): HyprUIMessage[] {
  return messages
    .filter((message) => !shouldHidePersistedMessage(message))
    .map((message) => message.message);
}
