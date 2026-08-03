import { useCallback } from "react";

import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { createFallbackChatTitle, generateChatTitle } from "./chat-title";
import {
  clearFailedChatGroupCreate,
  markFailedChatGroupCreate,
  trackPendingChatPersist,
} from "./pending-persists";
import { buildPersistedChatMessage } from "./persisted-messages";
import {
  createChatGroupWithMessage,
  setChatGroupTitleIfCurrent,
  upsertChatMessage,
} from "./queries";

import { useLanguageModel } from "~/ai/hooks";
import type { ContextRef } from "~/chat/context/entities";
import type { ChatMessageSender, ChatScope, AnlgUIMessage } from "~/chat/types";
import { useOwnerUserId } from "~/shared/owner-user";
import { id } from "~/shared/utils";

// Local writes normally land in milliseconds; retries cover transient
// "database is locked" contention so a send does not lose its turn to a
// momentary lock.
const PERSIST_RETRY_DELAYS_MS = [120, 360];

async function persistWithRetry(run: () => Promise<unknown>) {
  for (let attempt = 0; ; attempt++) {
    try {
      await run();
      return;
    } catch (error) {
      if (attempt >= PERSIST_RETRY_DELAYS_MS.length) {
        throw error;
      }
      await new Promise((resolve) =>
        setTimeout(resolve, PERSIST_RETRY_DELAYS_MS[attempt]),
      );
    }
  }
}

export function useChatActions({
  chatScope,
  groupId,
  onGroupCreated,
  onGroupCreateFailed,
}: {
  chatScope: ChatScope;
  groupId: string | undefined;
  onGroupCreated: (newGroupId: string) => void;
  onGroupCreateFailed?: (failedGroupId: string) => void;
}) {
  const ownerUserId = useOwnerUserId();
  const titleModel = useLanguageModel("title");

  const queueChatTitleGeneration = useCallback(
    (params: {
      groupId: string;
      fallbackTitle: string;
      initialRequest: string;
    }) => {
      const { groupId, fallbackTitle, initialRequest } = params;

      if (!titleModel || !initialRequest.trim()) {
        return null;
      }

      return generateChatTitle({
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
      parts: AnlgUIMessage["parts"],
      sendMessage: ChatMessageSender,
      contextRefs?: ContextRef[],
    ) => {
      if (!ownerUserId) {
        console.error("Cannot persist chat message without an owner user id");
        return;
      }

      const messageId = id();
      const metadata = {
        chatScope,
        createdAt: Date.now(),
        ...(contextRefs && contextRefs.length > 0 ? { contextRefs } : {}),
      };
      const uiMessage: AnlgUIMessage = {
        id: messageId,
        role: "user",
        parts,
        metadata,
      };

      const currentGroupId = groupId ?? id();
      const message = buildPersistedChatMessage({
        message: uiMessage,
        chatGroupId: currentGroupId,
        ownerUserId,
        status: "ready",
        content,
      });
      const fallbackTitle = groupId
        ? undefined
        : createFallbackChatTitle(content);
      const runPersist = fallbackTitle
        ? () =>
            createChatGroupWithMessage({
              groupId: currentGroupId,
              ownerUserId,
              title: fallbackTitle,
              createdAt: message.createdAt,
              message,
            })
        : () => upsertChatMessage(message);

      sendMessage(uiMessage, {
        chatGroupId: currentGroupId,
        beforeSend: async (trackCompletion) => {
          const persist = persistWithRetry(runPersist);
          trackPendingChatPersist(currentGroupId, persist);
          try {
            await persist;
            if (fallbackTitle) {
              clearFailedChatGroupCreate(currentGroupId);
              onGroupCreated(currentGroupId);
              const titleCompletion = queueChatTitleGeneration({
                groupId: currentGroupId,
                fallbackTitle,
                initialRequest: content,
              });
              if (titleCompletion) {
                trackCompletion(titleCompletion);
              }
            }
          } catch (error) {
            console.error("Failed to persist outgoing chat message", error);
            sonnerToast.error("Could not save this chat message.");
            if (fallbackTitle) {
              markFailedChatGroupCreate(currentGroupId);
              onGroupCreateFailed?.(currentGroupId);
            }
            throw error;
          }
        },
      });
    },
    [
      chatScope,
      groupId,
      ownerUserId,
      onGroupCreated,
      onGroupCreateFailed,
      queueChatTitleGeneration,
    ],
  );

  return { handleSendMessage };
}
