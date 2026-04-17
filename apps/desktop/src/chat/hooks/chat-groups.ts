import { useMemo } from "react";

import * as main from "~/store/tinybase/store/main";

export type ChatGroup = {
  id: string;
  title: string;
  created_at: string;
};

export function useChatGroupTitle(groupId: string | undefined): string {
  const title = main.UI.useCell(
    "chat_groups",
    groupId || "",
    "title",
    main.STORE_ID,
  );
  return (title as string | undefined) ?? "";
}

export function useChatGroup(groupId: string): ChatGroup | null {
  const row = main.UI.useRow("chat_groups", groupId, main.STORE_ID);
  return useMemo(() => {
    if (!row || Object.keys(row).length === 0) return null;
    return {
      id: groupId,
      title: String(row.title ?? ""),
      created_at: String(row.created_at ?? ""),
    };
  }, [groupId, row]);
}

export function useRecentChatGroupIds(limit: number): string[] {
  return main.UI.useSortedRowIds(
    "chat_groups",
    "created_at",
    true,
    0,
    limit,
    main.STORE_ID,
  );
}
