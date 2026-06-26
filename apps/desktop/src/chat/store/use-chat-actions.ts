import { useCallback } from "react";

import { createFallbackChatTitle, generateChatTitle } from "./chat-title";
import { useCreateChatMessage } from "./useCreateChatMessage";

import { useLanguageModel } from "~/ai/hooks";
import type { ContextRef } from "~/chat/context/entities";
import type { HyprUIMessage } from "~/chat/types";
import { id } from "~/shared/utils";
import * as main from "~/store/tinybase/store/main";

export function useChatActions({
  groupId,
  onGroupCreated,
}: {
  groupId: string | undefined;
  onGroupCreated: (newGroupId: string) => void;
}) {
  const { user_id } = main.UI.useValues(main.STORE_ID);
  const store = main.UI.useStore(main.STORE_ID);
  const titleModel = useLanguageModel("title");

  const createChatGroup = main.UI.useSetRowCallback(
    "chat_groups",
    (p: { groupId: string; title: string }) => p.groupId,
    (p: { groupId: string; title: string }) => ({
      user_id,
      created_at: new Date().toISOString(),
      title: p.title,
    }),
    [user_id],
    main.STORE_ID,
  );

  const setChatGroupTitle = main.UI.useSetCellCallback(
    "chat_groups",
    (p: { groupId: string; title: string }) => p.groupId,
    "title",
    (p: { groupId: string; title: string }) => p.title,
    [],
    main.STORE_ID,
  );

  const createChatMessage = useCreateChatMessage();

  const queueChatTitleGeneration = useCallback(
    (params: {
      groupId: string;
      fallbackTitle: string;
      initialRequest: string;
    }) => {
      const { groupId, fallbackTitle, initialRequest } = params;

      if (!titleModel || !initialRequest.trim()) {
        return;
      }

      void generateChatTitle({
        model: titleModel,
        initialRequest,
      })
        .then((title) => {
          if (!title) {
            return;
          }

          const currentTitle = store?.getCell("chat_groups", groupId, "title");

          if (currentTitle !== fallbackTitle) {
            return;
          }

          setChatGroupTitle({ groupId, title });
        })
        .catch((error) => {
          console.error("Failed to generate chat title", error);
        });
    },
    [setChatGroupTitle, store, titleModel],
  );

  const handleSendMessage = useCallback(
    (
      content: string,
      parts: HyprUIMessage["parts"],
      sendMessage: (
        message: HyprUIMessage,
        options?: { chatGroupId?: string },
      ) => void,
      contextRefs?: ContextRef[],
    ) => {
      const messageId = id();
      const metadata = {
        createdAt: Date.now(),
        ...(contextRefs && contextRefs.length > 0 ? { contextRefs } : {}),
      };
      const uiMessage: HyprUIMessage = {
        id: messageId,
        role: "user",
        parts,
        metadata,
      };

      let currentGroupId = groupId;
      if (!currentGroupId) {
        currentGroupId = id();
        const fallbackTitle = createFallbackChatTitle(content);
        createChatGroup({ groupId: currentGroupId, title: fallbackTitle });
        onGroupCreated(currentGroupId);
        queueChatTitleGeneration({
          groupId: currentGroupId,
          fallbackTitle,
          initialRequest: content,
        });
      }

      createChatMessage({
        id: messageId,
        chat_group_id: currentGroupId,
        content,
        role: "user",
        parts,
        metadata,
      });

      sendMessage(uiMessage, { chatGroupId: currentGroupId });
    },
    [
      groupId,
      createChatGroup,
      createChatMessage,
      onGroupCreated,
      queueChatTitleGeneration,
    ],
  );

  return { handleSendMessage };
}
