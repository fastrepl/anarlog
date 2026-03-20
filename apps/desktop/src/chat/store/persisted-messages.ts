import type { ChatMessageStatus, ChatMessageStorage } from "@hypr/store";

import type { HyprUIMessage } from "~/chat/types";
import * as main from "~/store/tinybase/store/main";

type ChatStore = NonNullable<ReturnType<typeof main.UI.useStore>>;

export type PersistedChatMessage = {
  id: string;
  row: ChatMessageStorage;
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

export function extractTextContent(parts: HyprUIMessage["parts"]) {
  return parts
    .filter((part): part is Extract<typeof part, { type: "text" }> => {
      return part.type === "text";
    })
    .map((part) => part.text)
    .join("");
}

function getCreatedAt(
  message: HyprUIMessage,
  existingRow?: Partial<ChatMessageStorage>,
) {
  if (existingRow?.created_at) {
    return existingRow.created_at;
  }

  const createdAt = message.metadata?.createdAt;
  if (typeof createdAt === "number") {
    return new Date(createdAt).toISOString();
  }

  return new Date().toISOString();
}

export function buildPersistedChatMessageRow({
  message,
  chatGroupId,
  userId,
  status,
  existingRow,
}: {
  message: HyprUIMessage;
  chatGroupId: string;
  userId: string;
  status: ChatMessageStatus;
  existingRow?: Partial<ChatMessageStorage>;
}): ChatMessageStorage {
  return {
    user_id: userId,
    created_at: getCreatedAt(message, existingRow),
    chat_group_id: chatGroupId,
    role: message.role,
    content: extractTextContent(message.parts),
    metadata: JSON.stringify(message.metadata ?? {}),
    parts: JSON.stringify(message.parts),
    status,
  };
}

export function rowToPersistedChatMessage(
  id: string,
  row: Record<string, unknown>,
): PersistedChatMessage {
  const status = normalizeChatMessageStatus(row.status);
  const message: HyprUIMessage = {
    id,
    role: row.role as "user" | "assistant",
    parts: parseJson(row.parts as string | undefined, []),
    metadata: parseJson(row.metadata as string | undefined, {}),
  };

  return {
    id,
    status,
    message,
    row: {
      user_id: String(row.user_id ?? ""),
      created_at: String(row.created_at ?? ""),
      chat_group_id: String(row.chat_group_id ?? ""),
      role: String(row.role ?? ""),
      content: String(row.content ?? ""),
      metadata: String(row.metadata ?? "{}"),
      parts: String(row.parts ?? "[]"),
      status,
    },
  };
}

export function getPersistedChatMessages(
  store: ChatStore,
  chatGroupId: string,
): PersistedChatMessage[] {
  const messages: PersistedChatMessage[] = [];

  store.forEachRow("chat_messages", (messageId, _forEachCell) => {
    const row = store.getRow("chat_messages", messageId);
    if (!row || row.chat_group_id !== chatGroupId) {
      return;
    }

    messages.push(rowToPersistedChatMessage(messageId as string, row));
  });

  return messages.sort(
    (a, b) =>
      new Date(a.row.created_at || 0).getTime() -
      new Date(b.row.created_at || 0).getTime(),
  );
}

export function shouldHidePersistedMessage(message: PersistedChatMessage) {
  return (
    message.message.role === "assistant" &&
    message.status === "streaming" &&
    message.message.parts.length === 0
  );
}

export function getVisibleChatMessages(
  store: ChatStore,
  chatGroupId: string,
): HyprUIMessage[] {
  return getPersistedChatMessages(store, chatGroupId)
    .filter((message) => !shouldHidePersistedMessage(message))
    .map((message) => message.message);
}
