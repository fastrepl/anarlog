import { useLingui } from "@lingui/react/macro";

import { cn } from "@anlg/utils";
import { createEditorTabs } from "@anlg/utils/session";

export { createEditorTabs } from "@anlg/utils/session";

import { HeaderViewEnhanced } from "./header-enhanced";
import { HeaderViewRaw } from "./header-raw";
import { HeaderViewShared } from "./header-shared-note";
import { HeaderViewTranscript } from "./header-transcript";

import { FolderPicker } from "~/session/components/folder-picker";
import {
  useCanShowTranscript,
  useHasTranscript,
} from "~/session/components/shared";
import { useEnhancedNotes } from "~/session/hooks/useEnhancedNotes";
import { deleteEnhancedNote } from "~/session/queries";
import { useSessionSharedNoteIds } from "~/shared-notes/event-keys";
import { type EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";

export function Header({ sessionId }: { sessionId: string }) {
  return <FolderPicker sessionId={sessionId} align="end" />;
}

export function SessionViewSwitcher({
  sessionId,
  editorTabs,
  currentTab,
  handleTabChange,
  isTranscribing = false,
  transcriptEditMode = false,
  onTranscriptEditModeChange,
}: {
  sessionId: string;
  editorTabs: EditorView[];
  currentTab: EditorView;
  handleTabChange: (view: EditorView) => void;
  isTranscribing?: boolean;
  transcriptEditMode?: boolean;
  onTranscriptEditModeChange?: (editMode: boolean) => void;
}) {
  const { t } = useLingui();
  const primaryEnhancedTabId = editorTabs.find(
    (view): view is Extract<EditorView, { type: "enhanced" }> =>
      view.type === "enhanced",
  )?.id;
  const shouldUseViewSwitcher = editorTabs.length > 1;

  if (!shouldUseViewSwitcher) {
    return null;
  }

  return (
    <div
      role="group"
      aria-label={t`Session note views`}
      data-tauri-drag-region="false"
      className={cn([
        "pointer-events-auto relative z-10 w-fit max-w-full shrink-0 overflow-visible",
        "bg-foreground/10 dark:bg-accent/55 rounded-pill flex h-7 items-center gap-[2px] p-[2px] [corner-shape:round]",
      ])}
    >
      {editorTabs.map((view, index) => {
        if (view.type === "enhanced") {
          return (
            <HeaderViewEnhanced
              key={`enhanced-${view.id}`}
              sessionId={sessionId}
              enhancedNoteId={view.id}
              canRemove={view.id !== primaryEnhancedTabId}
              onRemove={
                view.id !== primaryEnhancedTabId
                  ? () => {
                      const previousView = editorTabs[index - 1];
                      if (
                        currentTab.type === "enhanced" &&
                        currentTab.id === view.id &&
                        previousView
                      ) {
                        handleTabChange(previousView);
                      }

                      void deleteEnhancedNote(view.id).catch((error) => {
                        console.error(
                          "[session-header] failed to remove summary",
                          error,
                        );
                      });
                    }
                  : undefined
              }
              onSelectNote={(enhancedNoteId) =>
                handleTabChange({ type: "enhanced", id: enhancedNoteId })
              }
              isActive={
                currentTab.type === "enhanced" && currentTab.id === view.id
              }
              onClick={() => handleTabChange(view)}
            />
          );
        }

        if (view.type === "raw") {
          return (
            <HeaderViewRaw
              key={view.type}
              sessionId={sessionId}
              isActive={currentTab.type === view.type}
              standalone={!shouldUseViewSwitcher}
              onClick={() => handleTabChange(view)}
            />
          );
        }

        if (view.type === "transcript") {
          return (
            <HeaderViewTranscript
              key={view.type}
              sessionId={sessionId}
              isActive={currentTab.type === view.type}
              isTranscribing={isTranscribing}
              editMode={transcriptEditMode}
              onEditModeChange={onTranscriptEditModeChange}
              onClick={() => handleTabChange(view)}
            />
          );
        }

        if (view.type === "shared") {
          return (
            <HeaderViewShared
              key={`shared-${view.id}`}
              isActive={
                currentTab.type === "shared" && currentTab.id === view.id
              }
              onClick={() => handleTabChange(view)}
            />
          );
        }

        return null;
      })}
    </div>
  );
}

export function useEditorTabs({
  audioExists = false,
  sessionId,
}: {
  audioExists?: boolean;
  sessionId: string;
}): EditorView[] {
  const canShowTranscript = useCanShowTranscript(sessionId, { audioExists });
  const hasTranscript = useHasTranscript(sessionId);
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const enhancedNoteIds = useEnhancedNotes(
    sessionId,
    hasTranscript && sessionMode === "inactive",
  );

  const sharedNoteIds = useSessionSharedNoteIds(sessionId);

  return createEditorTabs({
    enhancedNoteIds,
    canShowTranscript,
    sharedNoteIds,
  });
}
