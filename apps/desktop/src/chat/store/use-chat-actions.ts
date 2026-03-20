import { useCallback, useRef } from "react";

import { useCreateChatMessage } from "./useCreateChatMessage";

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

  const createChatMessage = useCreateChatMessage();

  const handleSendMessage = useCallback(
    (
      content: string,
      parts: HyprUIMessage["parts"],
      sendMessage: (message: HyprUIMessage) => void,
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
        const title = content.slice(0, 50) + (content.length > 50 ? "..." : "");
        createChatGroup({ groupId: currentGroupId, title });
        onGroupCreated(currentGroupId);
      }

      createChatMessage({
        id: messageId,
        chat_group_id: currentGroupId,
        content,
        role: "user",
        parts,
        metadata,
      });

      sendMessage(uiMessage);
    },
    [groupId, createChatGroup, createChatMessage, onGroupCreated],
  );

  return { handleSendMessage };
}

export function useStableSessionId(groupId: string | undefined) {
  const sessionIdRef = useRef<string>(groupId ?? id());
  const lastGroupIdRef = useRef<string | undefined>(groupId);

  if (groupId !== lastGroupIdRef.current) {
    const isFirstGroupCreation =
      lastGroupIdRef.current === undefined && groupId !== undefined;
    lastGroupIdRef.current = groupId;

    if (!isFirstGroupCreation) {
      sessionIdRef.current = groupId ?? id();
    }
  }

  return sessionIdRef.current;
}
