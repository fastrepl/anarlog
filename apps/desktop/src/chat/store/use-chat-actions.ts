import { useCallback } from "react";

import { createFallbackChatTitle, generateChatTitle } from "./chat-title";
import { buildPersistedChatMessage } from "./persisted-messages";
import {
  createChatGroupWithMessage,
  setChatGroupTitleIfCurrent,
  upsertChatMessage,
} from "./queries";

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
  const titleModel = useLanguageModel("title");

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

          return setChatGroupTitleIfCurrent({
            groupId,
            expectedTitle: fallbackTitle,
            title,
          });
        })
        .catch((error) => {
          console.error("Failed to generate chat title", error);
        });
    },
    [titleModel],
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
      if (!user_id) {
        console.error("Cannot persist chat message without an owner user id");
        return;
      }

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

      const currentGroupId = groupId ?? id();
      const message = buildPersistedChatMessage({
        message: uiMessage,
        chatGroupId: currentGroupId,
        ownerUserId: user_id,
        status: "ready",
        content,
      });
      const fallbackTitle = groupId
        ? undefined
        : createFallbackChatTitle(content);
      const persist = fallbackTitle
        ? createChatGroupWithMessage({
            groupId: currentGroupId,
            ownerUserId: user_id,
            title: fallbackTitle,
            createdAt: message.createdAt,
            message,
          })
        : upsertChatMessage(message);

      void persist
        .then(() => {
          if (fallbackTitle) {
            onGroupCreated(currentGroupId);
            queueChatTitleGeneration({
              groupId: currentGroupId,
              fallbackTitle,
              initialRequest: content,
            });
          }

          sendMessage(uiMessage, { chatGroupId: currentGroupId });
        })
        .catch((error) => {
          console.error("Failed to persist outgoing chat message", error);
        });
    },
    [groupId, user_id, onGroupCreated, queueChatTitleGeneration],
  );

  return { handleSendMessage };
}
