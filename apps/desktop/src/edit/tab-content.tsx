import { MultiFileDiff } from "@pierre/diffs/react";
import { useCallback, useMemo } from "react";

import { useStrictModeUnmount } from "./hooks";

import { usePendingEditStore } from "~/chat/tools/pending-edit-store";
import { StandardContentWrapper } from "~/shared/main";
import * as main from "~/store/tinybase/store/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";

type EditTab = Extract<Tab, { type: "edit" }>;

export function TabContentEdit({ tab }: { tab: EditTab }) {
  const edit = usePendingEditStore((s) => s.edits.get(tab.requestId));
  const resolveEdit = usePendingEditStore((s) => s.resolveEdit);

  const store = main.UI.useStore(main.STORE_ID);
  const sessionTitle = useMemo(() => {
    if (!store || !edit) return null;
    const title = store.getCell("sessions", edit.sessionId, "title");
    return typeof title === "string" && title.trim() ? title : null;
  }, [store, edit]);
  const summaryTitle = useMemo(() => {
    if (!store || !edit) return null;
    const title = store.getCell("enhanced_notes", edit.enhancedNoteId, "title");
    return typeof title === "string" && title.trim() ? title : null;
  }, [store, edit]);

  const declineOnUnmount = useCallback(() => {
    const still = usePendingEditStore.getState().edits.get(tab.requestId);
    if (still) {
      usePendingEditStore.getState().resolveEdit(tab.requestId, false);
    }
  }, [tab.requestId]);
  useStrictModeUnmount(declineOnUnmount);

  const oldFile = useMemo(
    () =>
      edit ? { name: "summary.md", contents: edit.currentContent || "" } : null,
    [edit],
  );
  const newFile = useMemo(
    () =>
      edit ? { name: "summary.md", contents: edit.proposedContent } : null,
    [edit],
  );

  if (!edit) {
    return (
      <StandardContentWrapper>
        <div className="text-muted-foreground flex h-full items-center justify-center">
          This edit is no longer pending.
        </div>
      </StandardContentWrapper>
    );
  }

  return (
    <StandardContentWrapper>
      <div className="flex h-full flex-col">
        <div className="border-border flex items-start justify-between gap-3 border-b px-4 py-3">
          <div className="min-w-0 flex-1">
            <div className="text-foreground text-[13px] font-medium">
              {sessionTitle ?? "Untitled session"}
            </div>
            <div className="text-muted-foreground text-[12px]">
              {summaryTitle ?? "Summary"}
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <button
              className="border-border bg-card text-muted-foreground hover:bg-accent rounded-md border px-4 py-1.5 text-[13px] transition-colors"
              onClick={() => {
                resolveEdit(tab.requestId, false);
                useTabs.getState().close(tab);
              }}
            >
              Decline
            </button>
            <button
              className="bg-primary text-primary-foreground hover:bg-primary/90 rounded-md px-4 py-1.5 text-[13px] transition-colors"
              onClick={() => {
                resolveEdit(tab.requestId, true);
                useTabs.getState().close(tab);
              }}
              autoFocus
            >
              Approve
            </button>
          </div>
        </div>
        <div className="flex-1 overflow-auto">
          <MultiFileDiff
            oldFile={oldFile!}
            newFile={newFile!}
            options={{ diffStyle: "unified" }}
          />
        </div>
      </div>
    </StandardContentWrapper>
  );
}
