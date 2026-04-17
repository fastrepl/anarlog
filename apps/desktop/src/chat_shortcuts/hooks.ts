import { useCallback, useMemo } from "react";

import type { ChatShortcut } from "@hypr/store";

import * as main from "~/store/tinybase/store/main";

export type UserShortcut = ChatShortcut & { id: string };

export function useChatShortcuts(): UserShortcut[] {
  const shortcuts = main.UI.useResultTable(
    main.QUERIES.visibleChatShortcuts,
    main.STORE_ID,
  );

  return useMemo(() => {
    return Object.entries(shortcuts as Record<string, ChatShortcut>).map(
      ([id, shortcut]) => ({
        id,
        ...shortcut,
      }),
    );
  }, [shortcuts]);
}

export function useChatShortcutCell(
  id: string,
  field: "title" | "content",
): string {
  const value = main.UI.useCell("chat_shortcuts", id, field, main.STORE_ID);
  return (value as string | undefined) ?? "";
}

export function useCreateChatShortcut(): (args: {
  title: string;
  content: string;
}) => string | null {
  const { user_id } = main.UI.useValues(main.STORE_ID);

  const setRow = main.UI.useSetRowCallback(
    "chat_shortcuts",
    (p: {
      id: string;
      user_id: string;
      created_at: string;
      title: string;
      content: string;
    }) => p.id,
    (p: {
      id: string;
      user_id: string;
      created_at: string;
      title: string;
      content: string;
    }) => ({
      user_id: p.user_id,
      created_at: p.created_at,
      title: p.title,
      content: p.content,
    }),
    [],
    main.STORE_ID,
  );

  return useCallback(
    ({ title, content }) => {
      if (!user_id) return null;
      const id = crypto.randomUUID();
      setRow({
        id,
        user_id,
        created_at: new Date().toISOString(),
        title,
        content,
      });
      return id;
    },
    [user_id, setRow],
  );
}

export function useUpdateChatShortcut(
  id: string,
): (row: { title?: string; content?: string }) => void {
  return main.UI.useSetPartialRowCallback(
    "chat_shortcuts",
    id,
    (row: { title?: string; content?: string }) => row,
    [id],
    main.STORE_ID,
  );
}

export function useDeleteChatShortcut(id: string): () => void {
  return main.UI.useDelRowCallback("chat_shortcuts", () => id, main.STORE_ID);
}
