import type { ChatMessage, ChatMessageStorage } from "@hypr/store";

import * as main from "~/store/tinybase/store/main";

export function useCreateChatMessage() {
  const { user_id } = main.UI.useValues(main.STORE_ID);

  return main.UI.useSetRowCallback(
    "chat_messages",
    (p: Omit<ChatMessage, "user_id" | "created_at"> & { id: string }) => p.id,
    (p: Omit<ChatMessage, "user_id" | "created_at"> & { id: string }) =>
      ({
        user_id,
        chat_group_id: p.chat_group_id,
        content: p.content,
        created_at: new Date().toISOString(),
        role: p.role,
        metadata: JSON.stringify(p.metadata),
        parts: JSON.stringify(p.parts),
      }) satisfies ChatMessageStorage,
    [user_id],
    main.STORE_ID,
  );
}
