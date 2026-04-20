import type { sessions } from "@hypr/db";
import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { Textarea } from "@hypr/ui/components/ui/textarea";

type SessionRow = typeof sessions.$inferSelect;

export type SessionEditorDraft = {
  title: string;
  rawMd: string;
};

export function SessionEditorView({
  session,
  draft,
  hasChanges,
  isSaving,
  isDeleting,
  onSubmit,
  onDelete,
  onDraftChange,
}: {
  session: SessionRow;
  draft: SessionEditorDraft;
  hasChanges: boolean;
  isSaving: boolean;
  isDeleting: boolean;
  onSubmit: (draft: SessionEditorDraft) => void;
  onDelete: () => void;
  onDraftChange: (
    update: (current: SessionEditorDraft) => SessionEditorDraft,
  ) => void;
}) {
  return (
    <form
      className="flex h-full flex-col gap-4 p-6"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit(draft);
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
          <Button type="submit" size="sm" disabled={!hasChanges || isSaving}>
            {isSaving ? "Saving…" : "Save"}
          </Button>
          <Button
            type="button"
            size="sm"
            variant="destructive"
            disabled={isDeleting}
            onClick={onDelete}
          >
            {isDeleting ? "Deleting…" : "Delete"}
          </Button>
        </div>
      </div>

      <label className="flex flex-col gap-1 text-sm text-neutral-700">
        <span className="text-xs font-medium text-neutral-500">Title</span>
        <Input
          value={draft.title}
          onChange={(event) =>
            onDraftChange((current) => ({
              ...current,
              title: event.target.value,
            }))
          }
          placeholder="Untitled session"
        />
      </label>

      <label className="flex min-h-0 flex-1 flex-col gap-1 text-sm text-neutral-700">
        <span className="text-xs font-medium text-neutral-500">Notes</span>
        <Textarea
          value={draft.rawMd}
          onChange={(event) =>
            onDraftChange((current) => ({
              ...current,
              rawMd: event.target.value,
            }))
          }
          placeholder="Write markdown here…"
          className="h-full min-h-0 flex-1 resize-none font-mono"
        />
      </label>
    </form>
  );
}

export function SessionEditorLoadingView() {
  return (
    <div className="grid h-full place-content-center text-sm text-neutral-500">
      Loading session…
    </div>
  );
}

export function SessionEditorMissingView() {
  return (
    <div className="grid h-full place-content-center text-sm text-neutral-500">
      This session does not exist.
    </div>
  );
}
