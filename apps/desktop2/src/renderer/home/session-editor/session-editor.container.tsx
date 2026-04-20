import { useMutation } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";

import { eq, sessions } from "@hypr/db";
import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { Textarea } from "@hypr/ui/components/ui/textarea";

import { db, useDrizzleLiveQuery } from "~/db";
import { uniqueIdFromTab, useTabsStore } from "~/tabs";

export function SessionEditorContainer({ sessionId }: { sessionId: string }) {
  const closeTab = useTabsStore((state) => state.close);

  const sessionQuery = useDrizzleLiveQuery<typeof sessions.$inferSelect>(
    db.select().from(sessions).where(eq(sessions.id, sessionId)),
  );
  const session = sessionQuery.data?.[0] ?? null;

  const updateMutation = useMutation({
    mutationFn: async (input: { title: string; rawMd: string }) => {
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

  const [draft, setDraft] = useState({ title: "", rawMd: "" });

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
    return (
      <div className="grid h-full place-content-center text-sm text-neutral-500">
        Loading session…
      </div>
    );
  }

  if (!session) {
    return (
      <div className="grid h-full place-content-center text-sm text-neutral-500">
        This session does not exist.
      </div>
    );
  }

  return (
    <form
      className="flex h-full flex-col gap-4 p-6"
      onSubmit={(event) => {
        event.preventDefault();
        void updateMutation.mutateAsync(draft);
      }}
    >
      <div className="flex items-center justify-between gap-4">
        <div className="flex flex-col gap-0.5">
          <div className="text-[10px] tracking-[0.12em] text-neutral-500 uppercase">
            Session
          </div>
          <div className="text-sm text-neutral-500">
            Updated {new Date(session.updatedAt).toLocaleString()}
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button
            type="submit"
            size="sm"
            disabled={!hasChanges || updateMutation.isPending}
          >
            {updateMutation.isPending ? "Saving…" : "Save"}
          </Button>
          <Button
            type="button"
            size="sm"
            variant="destructive"
            disabled={deleteMutation.isPending}
            onClick={() => {
              void deleteMutation.mutateAsync();
            }}
          >
            {deleteMutation.isPending ? "Deleting…" : "Delete"}
          </Button>
        </div>
      </div>

      <label className="flex flex-col gap-1 text-sm text-neutral-700">
        <span className="text-xs font-medium text-neutral-500">Title</span>
        <Input
          value={draft.title}
          onChange={(event) =>
            setDraft((current) => ({ ...current, title: event.target.value }))
          }
          placeholder="Untitled session"
        />
      </label>

      <label className="flex min-h-0 flex-1 flex-col gap-1 text-sm text-neutral-700">
        <span className="text-xs font-medium text-neutral-500">Notes</span>
        <Textarea
          value={draft.rawMd}
          onChange={(event) =>
            setDraft((current) => ({ ...current, rawMd: event.target.value }))
          }
          placeholder="Write markdown here…"
          className="h-full min-h-0 flex-1 resize-none font-mono"
        />
      </label>
    </form>
  );
}
