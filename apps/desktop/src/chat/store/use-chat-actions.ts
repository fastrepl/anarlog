import { useCallback } from "react";

import { sonnerToast } from "@hypr/ui/components/ui/toast";

import { createFallbackChatTitle, generateChatTitle } from "./chat-title";
import {
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
import type { HyprUIMessage } from "~/chat/types";
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
  groupId,
  onGroupCreated,
  onGroupCreateFailed,
}: {
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
      if (!ownerUserId) {
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

      sendMessage(uiMessage, { chatGroupId: currentGroupId });
      if (fallbackTitle) {
        onGroupCreated(currentGroupId);
      }

      const persist = persistWithRetry(runPersist);
      trackPendingChatPersist(currentGroupId, persist);
      void persist
        .then(() => {
          if (fallbackTitle) {
            queueChatTitleGeneration({
              groupId: currentGroupId,
              fallbackTitle,
              initialRequest: content,
            });
          }
        })
        .catch((error) => {
          console.error("Failed to persist outgoing chat message", error);
          sonnerToast.error("Could not save this chat message.");
          if (fallbackTitle) {
            // The group row was never created; leaving the shell pointed at
            // it would orphan every follow-up message, and later persists
            // into it would create rows that never appear in history.
            markFailedChatGroupCreate(currentGroupId);
            onGroupCreateFailed?.(currentGroupId);
          }
        });
    },
    [
      groupId,
      ownerUserId,
      onGroupCreated,
      onGroupCreateFailed,
      queueChatTitleGeneration,
    ],
  );

  return { handleSendMessage };
}
