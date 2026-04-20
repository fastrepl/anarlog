import "../note-editor.css";

import { useMutation } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";

import { dailyNotes, eq } from "@hypr/db";

import { db, useDrizzleLiveQuery } from "~/db";
import { DailyNoteEditorView } from "~/home/daily-note-editor/daily-note-editor.view";

export function DailyNoteEditorContainer({ date }: { date: string }) {
  const noteQuery = useDrizzleLiveQuery<typeof dailyNotes.$inferSelect>(
    db.select().from(dailyNotes).where(eq(dailyNotes.date, date)),
  );
  const loadedContent = noteQuery.data?.[0]?.content ?? "";
  const [draft, setDraft] = useState(loadedContent);

  useEffect(() => {
    setDraft(loadedContent);
  }, [loadedContent]);

  const upsertMutation = useMutation({
    mutationFn: async (content: string) => {
      const now = new Date().toISOString();
      await db
        .insert(dailyNotes)
        .values({
          date,
          content,
          createdAt: now,
          updatedAt: now,
        })
        .onConflictDoUpdate({
          target: dailyNotes.date,
          set: { content, updatedAt: now },
        });
    },
  });
  const { isPending, mutateAsync } = upsertMutation;

  useEffect(() => {
    if (noteQuery.isLoading || isPending || draft === loadedContent) {
      return;
    }

    const timeout = window.setTimeout(() => {
      void mutateAsync(draft);
    }, 300);

    return () => window.clearTimeout(timeout);
  }, [draft, isPending, loadedContent, mutateAsync, noteQuery.isLoading]);

  const status = useMemo<"idle" | "saving">(
    () => (isPending ? "saving" : "idle"),
    [isPending],
  );

  return (
    <DailyNoteEditorView value={draft} onChange={setDraft} status={status} />
  );
}
