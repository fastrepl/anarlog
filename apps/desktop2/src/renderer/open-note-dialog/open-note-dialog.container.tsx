import { useMemo } from "react";

import { useDrizzleLiveQuery } from "~/db";
import {
  OpenNoteDialogView,
  type OpenNoteDialogItem,
} from "~/open-note-dialog/open-note-dialog.view";
import { sessionsListQuery, type SessionSummaryRow } from "~/sessions";
import { useTabsStore } from "~/tabs";

export function OpenNoteDialogContainer({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const openNew = useTabsStore((state) => state.openNew);
  const sessionsQuery = useDrizzleLiveQuery<SessionSummaryRow>(
    sessionsListQuery,
    { enabled: open },
  );

  const items = useMemo<OpenNoteDialogItem[]>(() => {
    return (sessionsQuery.data ?? []).map((session) => ({
      id: session.id,
      title: session.title || "Untitled session",
      subtitle: new Date(session.updatedAt).toLocaleString(),
    }));
  }, [sessionsQuery.data]);

  return (
    <OpenNoteDialogView
      open={open}
      items={items}
      onOpenChange={onOpenChange}
      onSelect={(id) => {
        openNew({ type: "sessions", id });
        onOpenChange(false);
      }}
    />
  );
}
