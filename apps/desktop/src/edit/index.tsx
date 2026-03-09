import { MultiFileDiff } from "@pierre/diffs/react";
import { PencilIcon } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef } from "react";

import { usePendingEditStore } from "~/chat/tools/pending-edit-store";
import { StandardTabWrapper } from "~/shared/main";
import { TabItemBase, type TabItem } from "~/shared/tabs";
import * as main from "~/store/tinybase/store/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";

function useStrictModeUnmount(fn: () => void) {
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      setTimeout(() => {
        if (!mountedRef.current) fn();
      }, 50);
    };
  }, [fn]);
}

type EditTab = Extract<Tab, { type: "edit" }>;

export const TabItemEdit: TabItem<EditTab> = ({
  tab,
  tabIndex,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
}) => {
  return (
    <TabItemBase
      icon={<PencilIcon className="h-4 w-4" />}
      title="Review Edit"
      selected={tab.active}
      pinned={tab.pinned}
      tabIndex={tabIndex}
      handleCloseThis={() => handleCloseThis(tab)}
      handleSelectThis={() => handleSelectThis(tab)}
      handleCloseOthers={handleCloseOthers}
      handleCloseAll={handleCloseAll}
      handlePinThis={() => handlePinThis(tab)}
      handleUnpinThis={() => handleUnpinThis(tab)}
    />
  );
};

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
      <StandardTabWrapper>
        <div className="flex h-full items-center justify-center text-neutral-400">
          This edit is no longer pending.
        </div>
      </StandardTabWrapper>
    );
  }

  return (
    <StandardTabWrapper>
      <div className="flex h-full flex-col">
        <div className="flex flex-col gap-0.5 border-b border-neutral-200 px-4 py-3">
          <div className="text-[13px] font-medium text-neutral-900">
            {sessionTitle ?? "Untitled session"}
          </div>
          <div className="text-[12px] text-neutral-500">
            {summaryTitle ?? "Summary"}
          </div>
        </div>
        <div className="flex-1 overflow-auto">
          <MultiFileDiff
            oldFile={oldFile!}
            newFile={newFile!}
            options={{ diffStyle: "unified" }}
          />
        </div>
        <div className="flex items-center justify-end gap-2 border-t border-neutral-200 px-4 py-3">
          <button
            className="rounded-md border border-neutral-300 bg-white px-4 py-1.5 text-[13px] text-neutral-600 transition-colors hover:bg-neutral-50"
            onClick={() => {
              resolveEdit(tab.requestId, false);
              useTabs.getState().close(tab);
            }}
          >
            Decline
          </button>
          <button
            className="rounded-md bg-neutral-800 px-4 py-1.5 text-[13px] text-white transition-colors hover:bg-neutral-700"
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
    </StandardTabWrapper>
  );
}
