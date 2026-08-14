import { MultiFileDiff } from "@pierre/diffs/react";
import { useCallback, useMemo } from "react";

import { useStrictModeUnmount } from "./hooks";

import { usePendingEditStore } from "~/chat/tools/pending-edit-store";
import { useEnhancedNote, useSessionSummary } from "~/session/queries";
import { StandardContentWrapper } from "~/shared/main";
import type { Tab } from "~/store/zustand/tabs";

type EditTab = Extract<Tab, { type: "edit" }>;

export function TabContentEdit({ tab }: { tab: EditTab }) {
  const edit = usePendingEditStore((s) => s.edits.get(tab.requestId));

  const session = useSessionSummary(edit?.sessionId ?? "");
  const summary = useEnhancedNote(
    edit?.target.kind === "summary" ? edit.target.enhancedNoteId : "",
  );
  const sessionTitle = session?.title.trim() || null;
  const summaryTitle = summary?.title.trim() || null;
  const isMemo = edit?.target.kind === "memo";

  const declineOnUnmount = useCallback(() => {
    const still = usePendingEditStore.getState().edits.get(tab.requestId);
    if (still) {
      usePendingEditStore.getState().resolveEdit(tab.requestId, false);
    }
  }, [tab.requestId]);
  useStrictModeUnmount(declineOnUnmount);

  const oldFile = useMemo(
    () =>
      edit
        ? {
            name: isMemo ? "memo.md" : "summary.md",
            contents: edit.currentContent || "",
          }
        : null,
    [edit, isMemo],
  );
  const newFile = useMemo(
    () =>
      edit
        ? {
            name: isMemo ? "memo.md" : "summary.md",
            contents: edit.proposedContent,
          }
        : null,
    [edit, isMemo],
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
        <div className="border-border border-b px-4 py-3">
          <div className="min-w-0 flex-1">
            <div className="text-foreground text-[13px] font-medium">
              {sessionTitle ?? "Untitled session"}
            </div>
            <div className="text-muted-foreground text-[12px]">
              {isMemo ? "Memo" : (summaryTitle ?? "Summary")}
            </div>
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
