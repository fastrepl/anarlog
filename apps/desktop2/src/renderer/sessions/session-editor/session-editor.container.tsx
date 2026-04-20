import { useMutation } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";

import { eq, sessions } from "@hypr/db";

import { db, useDrizzleLiveQuery } from "~/db";
import {
  type SessionEditorDraft,
  SessionEditorLoadingView,
  SessionEditorMissingView,
  SessionEditorView,
} from "~/sessions/session-editor/session-editor.view";
import { uniqueIdFromTab, useTabsStore } from "~/tabs";

export function SessionEditorContainer({ sessionId }: { sessionId: string }) {
  const closeTab = useTabsStore((state) => state.close);

  const sessionQuery = useDrizzleLiveQuery<typeof sessions.$inferSelect>(
    db.select().from(sessions).where(eq(sessions.id, sessionId)),
  );
  const session = sessionQuery.data?.[0] ?? null;

  const updateMutation = useMutation({
    mutationFn: async (input: SessionEditorDraft) => {
      await db
        .update(sessions)
        .set({
          title: input.title,
          rawMd: input.rawMd,
          updatedAt: new Date().toISOString(),
        })
        .where(eq(sessions.id, sessionId));
    },
  });

  const deleteMutation = useMutation({
    mutationFn: async () => {
      await db.delete(sessions).where(eq(sessions.id, sessionId));
    },
    onSuccess: () => {
      closeTab(uniqueIdFromTab({ type: "sessions", id: sessionId }));
    },
  });

  const [draft, setDraft] = useState<SessionEditorDraft>({
    title: "",
    rawMd: "",
  });

  useEffect(() => {
    if (!session) {
      return;
    }

    setDraft({
      title: session.title,
      rawMd: session.rawMd,
    });
  }, [session?.id, session?.rawMd, session?.title]);

  const hasChanges = useMemo(() => {
    if (!session) {
      return false;
    }

    return draft.title !== session.title || draft.rawMd !== session.rawMd;
  }, [draft.rawMd, draft.title, session]);

  if (sessionQuery.isLoading) {
    return <SessionEditorLoadingView />;
  }

  if (!session) {
    return <SessionEditorMissingView />;
  }

  return (
    <SessionEditorView
      session={session}
      draft={draft}
      hasChanges={hasChanges}
      isSaving={updateMutation.isPending}
      isDeleting={deleteMutation.isPending}
      onSubmit={(nextDraft) => {
        void updateMutation.mutateAsync(nextDraft);
      }}
      onDelete={() => {
        void deleteMutation.mutateAsync();
      }}
      onDraftChange={setDraft}
    />
  );
}
