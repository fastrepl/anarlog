import { useMemo } from "react";

import { desc, sessions } from "@hypr/db";

import { db, useDrizzleLiveQuery } from "~/db";
import {
  OpenNoteDialogView,
  type OpenNoteDialogItem,
} from "~/open-note-dialog/open-note-dialog.view";
import { useTabsStore } from "~/tabs";

type SessionSummaryRow = {
  id: string;
  title: string;
  updatedAt: string;
};

const sessionsListQuery = db
  .select({
    id: sessions.id,
    title: sessions.title,
    updatedAt: sessions.updatedAt,
  })
  .from(sessions)
  .orderBy(desc(sessions.updatedAt), desc(sessions.id));

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
