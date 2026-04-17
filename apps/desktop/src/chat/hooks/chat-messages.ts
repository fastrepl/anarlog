import { useCallback } from "react";

import type { ChatMessageStatus } from "@hypr/store";

import {
  buildPersistedChatMessageRow,
  getPersistedChatMessages,
  getVisibleChatMessages,
} from "./persisted-messages";

import type { HyprUIMessage } from "~/chat/types";
import * as main from "~/store/tinybase/store/main";

export function useGetVisibleChatMessages() {
  const store = main.UI.useStore(main.STORE_ID);
  return useCallback(
    (chatGroupId: string): HyprUIMessage[] => {
      if (!store) return [];
      return getVisibleChatMessages(store, chatGroupId);
    },
    [store],
  );
}

export function useSetChatMessage() {
  const store = main.UI.useStore(main.STORE_ID);
  const { user_id: userId } = main.UI.useValues(main.STORE_ID);
  return useCallback(
    (params: {
      message: HyprUIMessage;
      chatGroupId: string;
      status: ChatMessageStatus;
    }) => {
      if (!store || !userId) return;
      const existingRow = store.getRow("chat_messages", params.message.id);
      store.setRow(
        "chat_messages",
        params.message.id,
        buildPersistedChatMessageRow({
          message: params.message,
          chatGroupId: params.chatGroupId,
          userId,
          status: params.status,
          existingRow,
        }),
      );
    },
    [store, userId],
  );
}

export function useDeleteLatestAssistantMessage() {
  const store = main.UI.useStore(main.STORE_ID);
  return useCallback(
    (chatGroupId: string) => {
      if (!store) return;
      const last = [...getPersistedChatMessages(store, chatGroupId)]
        .reverse()
        .find((m) => m.message.role === "assistant");
      if (last) store.delRow("chat_messages", last.id);
    },
    [store],
  );
}

export function useSyncChatMessages() {
  const store = main.UI.useStore(main.STORE_ID);
  return useCallback(
    (chatGroupId: string, keepIds: Set<string>) => {
      if (!store) return;
      store.transaction(() => {
        getPersistedChatMessages(store, chatGroupId).forEach(({ id }) => {
          if (!keepIds.has(id)) store.delRow("chat_messages", id);
        });
      });
    },
    [store],
  );
}
