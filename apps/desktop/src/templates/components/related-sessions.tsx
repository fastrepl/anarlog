import { StickyNote } from "lucide-react";
import { useMemo } from "react";

import { cn, formatDistanceToNow } from "@hypr/utils";

import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";

export function RelatedSessions({ templateId }: { templateId: string }) {
  const store = main.UI.useStore(main.STORE_ID);
  const openCurrent = useTabs((state) => state.openCurrent);
  const enhancedNoteIds = main.UI.useSliceRowIds(
    main.INDEXES.enhancedNotesByTemplate,
    templateId,
    main.STORE_ID,
  );

  const sessionIds = useMemo(() => {
    if (!store || !enhancedNoteIds) return [];

    const uniqueSessionIds = new Set<string>();
    for (const noteId of enhancedNoteIds) {
      const sessionId = store.getCell("enhanced_notes", noteId, "session_id");
      if (typeof sessionId === "string" && sessionId) {
        uniqueSessionIds.add(sessionId);
      }
    }
    return Array.from(uniqueSessionIds);
  }, [store, enhancedNoteIds]);

  if (sessionIds.length === 0) {
    return (
      <div className="rounded-lg border border-neutral-200 bg-neutral-50 p-4">
        <p className="text-sm text-neutral-500">
          Notes enhanced with this template will appear here.
        </p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      {sessionIds.map((sessionId) => (
        <RelatedSessionItem
          key={sessionId}
          sessionId={sessionId}
          onClick={() => openCurrent({ type: "sessions", id: sessionId })}
        />
      ))}
    </div>
  );
}

function RelatedSessionItem({
  sessionId,
  onClick,
}: {
  sessionId: string;
  onClick: () => void;
}) {
  const title = main.UI.useCell("sessions", sessionId, "title", main.STORE_ID);
  const createdAt = main.UI.useCell(
    "sessions",
    sessionId,
    "created_at",
    main.STORE_ID,
  );

  const timeAgo = useMemo(() => {
    if (!createdAt) return "";
    return formatDistanceToNow(new Date(String(createdAt)));
  }, [createdAt]);

  return (
    <button
      onClick={onClick}
      className={cn([
        "w-full px-3 py-2.5",
        "flex items-center gap-3",
        "hover:bg-neutral-50 active:bg-neutral-100",
        "rounded-lg border border-neutral-200 transition-colors",
        "text-left",
      ])}
    >
      <StickyNote className="h-4 w-4 shrink-0 text-neutral-400" />
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium text-neutral-900">
          {title || "Untitled"}
        </div>
        {timeAgo && <div className="text-xs text-neutral-500">{timeAgo}</div>}
      </div>
    </button>
  );
}
