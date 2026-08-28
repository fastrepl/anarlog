import { useLingui } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";
import type { CSSProperties } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import { HeaderViewEnhanced } from "./header-enhanced";
import { HeaderViewRaw } from "./header-raw";
import { HeaderViewTranscript } from "./header-transcript";

import { FolderPicker } from "~/session/components/folder-picker";
import { useCanShowTranscript } from "~/session/components/shared";
import { useEnsureDefaultSummary } from "~/session/hooks/useEnhancedNotes";
import { deleteEnhancedNote, useEnhancedNoteRecords } from "~/session/queries";
import { type EditorView } from "~/store/zustand/tabs/schema";

export function Header({ sessionId }: { sessionId: string }) {
  return <FolderPicker sessionId={sessionId} align="end" />;
}

export function SessionViewSwitcher({
  sessionId,
  editorTabs,
  currentTab,
  handleTabChange,
  isTranscribing = false,
}: {
  sessionId: string;
  editorTabs: EditorView[];
  currentTab: EditorView;
  handleTabChange: (view: EditorView) => void;
  isTranscribing?: boolean;
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
      {...mergeStyleXProps(styles.switcher, undefined, {
        cornerShape: "round",
      } as CSSProperties)}
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
  useEnsureDefaultSummary(sessionId);
  const canShowTranscript = useCanShowTranscript(sessionId, { audioExists });

  const enhancedNoteIds = useEnhancedNoteRecords(sessionId).map(
    (note) => note.id,
  );

  return createEditorTabs({
    enhancedNoteIds,
    canShowTranscript,
  });
}

export function createEditorTabs({
  enhancedNoteIds,
  canShowTranscript,
}: {
  enhancedNoteIds: string[];
  canShowTranscript: boolean;
}): EditorView[] {
  const enhancedTabs: EditorView[] = enhancedNoteIds.map((id) => ({
    type: "enhanced",
    id,
  }));

  return [
    ...enhancedTabs,
    { type: "raw" },
    ...(canShowTranscript ? [{ type: "transcript" } as const] : []),
  ];
}

const styles = stylex.create({
  switcher: {
    alignItems: "center",
    backgroundColor: {
      default: `color-mix(in oklab, ${colors.foreground} 10%, transparent)`,
      ":is(.dark *)": `color-mix(in oklab, ${colors.accent} 55%, transparent)`,
    },
    borderRadius: radii.full,
    display: "flex",
    flexShrink: 0,
    gap: "2px",
    height: "30px",
    maxWidth: "100%",
    overflow: "visible",
    padding: "2px",
    pointerEvents: "auto",
    position: "relative",
    width: "fit-content",
    zIndex: 10,
  },
});

export { styles as sessionViewSwitcherStyles };
