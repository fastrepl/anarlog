import { t } from "@lingui/core/macro";
import { ArrowDown, ArrowUp } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import {
  type MouseEvent as ReactMouseEvent,
  type RefObject,
  useCallback,
  useDeferredValue,
  useMemo,
  useRef,
  useState,
} from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";

import {
  getTranscriptContextSelection,
  getTranscriptMergeTarget,
  getTranscriptSectionKeyFromElement,
  getTranscriptSectionSelection,
  mergeTranscriptSelections,
  type TranscriptWordSelection,
} from "./selection";
import {
  TranscriptSelectionProvider,
  useTranscriptSelectionSources,
} from "./selection-context";
import { MultiSelectionBar, SelectionMenu } from "./selection-menu";
import type { TranscriptContextMenuRequest } from "./selection-menu";
import { TranscriptSeparator } from "./separator";
import { RenderTranscript } from "./transcript";
import {
  preserveScrollPosition,
  useAutoScroll,
  usePlaybackAutoScroll,
  useScrollDetection,
} from "./viewport-hooks";

import { trackAnalyticsEvent } from "~/analytics";
import { useAudioPlayer } from "~/audio-player";
import { useAudioTime } from "~/audio-player/provider";
import type { Segment } from "~/stt/live-segment";
import {
  assignTranscriptSpeaker,
  mergeTranscriptSegments,
} from "~/stt/queries";

const LIVE_TRANSCRIPT_PLACEHOLDER_ID = "__live-transcript__";

export function TranscriptViewer({
  transcriptIds,
  liveSegments,
  currentActive,
  captureGeneration = 0,
  scrollRef,
  editMode = false,
}: {
  transcriptIds: string[];
  liveSegments: Segment[];
  currentActive: boolean;
  captureGeneration?: number;
  scrollRef: RefObject<HTMLDivElement | null>;
  editMode?: boolean;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [scrollElement, setScrollElement] = useState<HTMLDivElement | null>(
    null,
  );
  const [contextRequest, setContextRequest] =
    useState<TranscriptContextMenuRequest | null>(null);
  const [selectedEntries, setSelectedEntries] = useState<
    Map<string, TranscriptWordSelection>
  >(() => new Map());
  const [selectionAnchor, setSelectionAnchor] = useState<string | null>(null);
  const [selectionForEditMode, setSelectionForEditMode] = useState(editMode);
  if (selectionForEditMode !== editMode) {
    setSelectionForEditMode(editMode);
    if (!editMode) {
      setSelectedEntries(new Map());
      setSelectionAnchor(null);
    }
  }
  const selectMode = editMode;
  const selectedKeys = useMemo(
    () => new Set(selectedEntries.keys()),
    [selectedEntries],
  );
  const { registerSource, collectEntries } = useTranscriptSelectionSources();
  const multiSelection = useMemo(
    () => mergeTranscriptSelections([...selectedEntries.values()]),
    [selectedEntries],
  );
  const handleContainerRef = useCallback(
    (node: HTMLDivElement | null) => {
      containerRef.current = node;
      setScrollElement(node);
      scrollRef.current = node;
    },
    [scrollRef],
  );

  const {
    isAtTop,
    isAtBottom,
    isNearBottom,
    canScroll,
    autoScrollEnabled,
    scrollToTop,
    scrollToBottom,
  } = useScrollDetection(containerRef, currentActive);

  const {
    state: playerState,
    pause,
    resume,
    start,
    seek,
    audioExists,
  } = useAudioPlayer();
  const time = useAudioTime();
  const deferredCurrentMs = useDeferredValue(time.current * 1000);
  const isPlaying = playerState === "playing";
  useHotkeys(
    "space",
    (e) => {
      e.preventDefault();
      if (playerState === "playing") {
        pause();
      } else if (playerState === "paused") {
        resume();
      } else if (playerState === "stopped") {
        start();
      }
    },
    { enableOnFormTags: false, enabled: !editMode },
  );

  usePlaybackAutoScroll(containerRef, deferredCurrentMs, isPlaying);
  const shouldAutoScroll = currentActive && autoScrollEnabled;
  const shouldScrollLastTranscriptToEnd = currentActive && isNearBottom;
  useAutoScroll(
    containerRef,
    [transcriptIds, liveSegments, shouldAutoScroll],
    shouldAutoScroll,
  );
  const visibleTranscriptIds =
    transcriptIds.length > 0
      ? transcriptIds
      : liveSegments.length > 0
        ? [LIVE_TRANSCRIPT_PLACEHOLDER_ID]
        : [];
  const visibleTranscriptIdsRef = useRef(visibleTranscriptIds);
  visibleTranscriptIdsRef.current = visibleTranscriptIds;

  const handleSelectionAction = useCallback(
    (action: "copy" | "play", selection: TranscriptWordSelection) => {
      if (action === "copy") {
        void navigator.clipboard.writeText(selection.text);
        return;
      }

      if (audioExists) {
        seek(selection.startMs / 1000);
        start();
      }
    },
    [audioExists, seek, start],
  );
  const handleAssignSpeaker = useCallback(
    async (selection: TranscriptWordSelection, humanId: string) => {
      await preserveScrollPosition(containerRef.current, () =>
        Promise.all(
          selection.groups.map((group) =>
            assignTranscriptSpeaker({
              transcriptId: group.transcriptId,
              segmentKey: group.segmentKey,
              humanId,
              anchorWordId: group.wordIds[0]!,
              mode: "segment",
              wordIds: group.wordIds,
            }),
          ),
        ),
      );
      trackAnalyticsEvent("participant_assigned", {
        assignment_scope: "selection",
        word_count: selection.groups.reduce(
          (count, group) => count + group.wordIds.length,
          0,
        ),
      });
    },
    [],
  );
  const handleMergeSegments = useCallback(async () => {
    const { order, entries } = collectEntries(visibleTranscriptIdsRef.current);
    const target = getTranscriptMergeTarget(
      new Set(selectedEntries.keys()),
      order,
      entries,
    );
    const targetGroup = target?.groups[0];
    const selection = mergeTranscriptSelections([...selectedEntries.values()]);
    if (!targetGroup || !selection) {
      return;
    }

    const groups = selection.groups.filter(
      (group) => group.transcriptId === targetGroup.transcriptId,
    );
    await preserveScrollPosition(containerRef.current, () =>
      Promise.all(
        groups.map((group) =>
          mergeTranscriptSegments({
            transcriptId: group.transcriptId,
            segmentKey: targetGroup.segmentKey,
            wordIds: group.wordIds,
          }),
        ),
      ),
    );
    trackAnalyticsEvent("participant_assigned", {
      assignment_scope: "merge",
      word_count: groups.reduce(
        (count, group) => count + group.wordIds.length,
        0,
      ),
    });
  }, [collectEntries, selectedEntries]);
  const canMergeSelection = useMemo(() => {
    if (selectedEntries.size < 2) {
      return false;
    }
    const { order, entries } = collectEntries(visibleTranscriptIds);
    return getTranscriptMergeTarget(selectedKeys, order, entries) != null;
  }, [
    collectEntries,
    selectedEntries.size,
    selectedKeys,
    visibleTranscriptIds,
  ]);
  const handleContextMenu = useCallback(
    (event: ReactMouseEvent<HTMLDivElement>) => {
      if (selectMode) {
        return;
      }
      const nativeSelection = window.getSelection();
      const activeRange =
        nativeSelection && nativeSelection.rangeCount > 0
          ? nativeSelection.getRangeAt(0)
          : undefined;
      const request = getTranscriptContextSelection({
        target: event.target,
        container: event.currentTarget,
        activeRange,
      });
      if (!request) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      setContextRequest({
        id: crypto.randomUUID(),
        range: request.range,
        selection: request.selection,
        x: event.clientX,
        y: event.clientY,
      });
    },
    [selectMode],
  );
  const handleContextClose = useCallback(() => {
    setContextRequest(null);
  }, []);
  const clearSelectedEntries = useCallback(() => {
    setSelectedEntries(new Map());
    setSelectionAnchor(null);
  }, []);

  useHotkeys(
    "esc",
    (event) => {
      event.preventDefault();
      clearSelectedEntries();
    },
    { enabled: editMode && selectedEntries.size > 0 },
  );

  const handleSegmentSelection = useCallback(
    (event: ReactMouseEvent<HTMLDivElement>) => {
      const target = event.target;
      const section =
        target instanceof Element
          ? target.closest<HTMLElement>("section[data-transcript-segment-id]")
          : null;
      if (!section || !event.currentTarget.contains(section)) {
        return;
      }
      if (
        target instanceof Element &&
        target.closest(
          "button, a, input, textarea, [data-transcript-speaker-assign], [data-transcript-editor]",
        )
      ) {
        return;
      }

      const hasSelectionModifier =
        event.metaKey || event.ctrlKey || event.shiftKey;
      if (!selectMode && !hasSelectionModifier) {
        if (selectedEntries.size > 0) {
          clearSelectedEntries();
        }
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      window.getSelection()?.removeAllRanges();
      const container = event.currentTarget;
      const targetKey = getTranscriptSectionKeyFromElement(section);
      if (!targetKey) {
        return;
      }
      const { order, entries } = collectEntries(
        visibleTranscriptIdsRef.current,
      );

      setSelectedEntries((current) => {
        const next = new Map(current);
        if (event.shiftKey && selectionAnchor) {
          const anchorIndex = order.indexOf(selectionAnchor);
          const targetIndex = order.indexOf(targetKey);
          if (anchorIndex !== -1 && targetIndex !== -1) {
            const start = Math.min(anchorIndex, targetIndex);
            const end = Math.max(anchorIndex, targetIndex);
            for (const key of order.slice(start, end + 1)) {
              const selection = entries.get(key);
              if (selection) {
                next.set(key, selection);
              }
            }
            return next;
          }
        }

        if (next.has(targetKey)) {
          next.delete(targetKey);
        } else {
          const selection =
            entries.get(targetKey) ??
            getTranscriptSectionSelection(section, container);
          if (selection) {
            next.set(targetKey, selection);
          }
        }
        return next;
      });
      setSelectionAnchor(targetKey);
    },
    [
      clearSelectedEntries,
      collectEntries,
      selectMode,
      selectedEntries.size,
      selectionAnchor,
    ],
  );

  return (
    <TranscriptSelectionProvider
      selectMode={selectMode}
      selectedKeys={selectedKeys}
      registerSource={registerSource}
    >
      <div {...stylex.props(styles.root)}>
        <div
          ref={handleContainerRef}
          data-transcript-container
          data-transcript-select-mode={selectMode ? "true" : undefined}
          onClickCapture={handleSegmentSelection}
          onContextMenu={handleContextMenu}
          {...stylex.props(styles.container)}
        >
          {visibleTranscriptIds.map((transcriptId, index) => {
            const isLastTranscript = index === visibleTranscriptIds.length - 1;
            const isActiveTranscript = currentActive && isLastTranscript;

            return (
              <div key={transcriptId} {...stylex.props(styles.transcript)}>
                <RenderTranscript
                  scrollElement={scrollElement}
                  isLastTranscript={isLastTranscript}
                  shouldScrollToEnd={shouldScrollLastTranscriptToEnd}
                  transcriptId={transcriptId}
                  currentActive={isActiveTranscript}
                  captureGeneration={isActiveTranscript ? captureGeneration : 0}
                  liveSegments={isActiveTranscript ? liveSegments : []}
                  currentMs={deferredCurrentMs}
                  seek={seek}
                  startPlayback={start}
                  audioExists={audioExists}
                  editMode={editMode}
                />
                {!isLastTranscript && <TranscriptSeparator />}
              </div>
            );
          })}

          <SelectionMenu
            containerRef={containerRef}
            contextRequest={contextRequest}
            audioExists={audioExists}
            onContextClose={handleContextClose}
            onAction={handleSelectionAction}
            onAssignSpeaker={handleAssignSpeaker}
          />
        </div>

        {multiSelection && (
          <MultiSelectionBar
            selection={multiSelection}
            entryCount={selectedEntries.size}
            canMerge={canMergeSelection}
            onClear={clearSelectedEntries}
            onAssignSpeaker={handleAssignSpeaker}
            onMerge={handleMergeSegments}
          />
        )}

        {canScroll && (
          <div
            data-transcript-scroll-controls
            {...stylex.props(styles.scrollControls)}
          >
            <button
              type="button"
              aria-label={t`Scroll to top`}
              onClick={scrollToTop}
              disabled={isAtTop}
              {...stylex.props(styles.scrollButton)}
            >
              <ArrowUp
                aria-hidden="true"
                {...stylex.props(styles.scrollIcon)}
              />
            </button>
            <div {...stylex.props(styles.scrollDivider)} />
            <button
              type="button"
              aria-label={t`Scroll to bottom`}
              onClick={scrollToBottom}
              disabled={isAtBottom}
              {...stylex.props(styles.scrollButton)}
            >
              <ArrowDown
                aria-hidden="true"
                {...stylex.props(styles.scrollIcon)}
              />
            </button>
          </div>
        )}
      </div>
    </TranscriptSelectionProvider>
  );
}

const styles = stylex.create({
  container: {
    display: {
      default: "flex",
      "::-webkit-scrollbar": "none",
    },
    flex: "1",
    flexDirection: "column",
    gap: "2rem",
    minHeight: 0,
    minWidth: 0,
    overflowX: "clip",
    overflowY: "auto",
    paddingBottom: "calc(4rem + env(safe-area-inset-bottom))",
    scrollPaddingBottom: "calc(8rem + env(safe-area-inset-bottom))",
    scrollbarWidth: "none",
  },
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    position: "relative",
  },
  scrollButton: {
    alignItems: "center",
    backgroundColor: {
      default: null,
      ":hover": `color-mix(in oklab, ${colors.muted} 55%, transparent)`,
      ":active": `color-mix(in oklab, ${colors.muted} 70%, transparent)`,
      ":focus-visible": `color-mix(in oklab, ${colors.muted} 55%, transparent)`,
    },
    display: "flex",
    height: "2rem",
    justifyContent: "center",
    opacity: {
      default: 1,
      ":disabled": 0.3,
    },
    outline: {
      default: null,
      ":focus-visible": "none",
    },
    pointerEvents: {
      default: null,
      ":disabled": "none",
    },
    width: "2rem",
  },
  scrollControls: {
    backdropFilter: {
      default: null,
      ":hover": "blur(12px)",
      ":focus-within": "blur(12px)",
    },
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in oklab, ${colors.background} 65%, transparent)`,
      ":focus-within": `color-mix(in oklab, ${colors.background} 65%, transparent)`,
    },
    borderColor: {
      default: "transparent",
      ":hover": `color-mix(in oklab, ${colors.border} 50%, transparent)`,
      ":focus-within": `color-mix(in oklab, ${colors.border} 50%, transparent)`,
    },
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: null,
      ":hover": shadows.sm,
      ":focus-within": shadows.sm,
    },
    color: {
      default: `color-mix(in oklab, ${colors.mutedForeground} 45%, transparent)`,
      ":hover": colors.foreground,
      ":focus-within": colors.foreground,
    },
    display: "flex",
    flexDirection: "column",
    overflow: "hidden",
    position: "absolute",
    right: "0.25rem",
    top: "50%",
    transform: "translateY(-50%)",
    transitionDuration: "150ms",
    transitionProperty:
      "background-color, border-color, color, box-shadow, backdrop-filter",
    zIndex: 40,
  },
  scrollDivider: {
    backgroundColor: {
      default: `color-mix(in oklab, ${colors.border} 20%, transparent)`,
      ":is([data-transcript-scroll-controls]:hover *)": `color-mix(in oklab, ${colors.border} 60%, transparent)`,
      ":is([data-transcript-scroll-controls]:focus-within *)": `color-mix(in oklab, ${colors.border} 60%, transparent)`,
    },
    height: "1px",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    width: "100%",
  },
  scrollIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  transcript: {
    display: "flex",
    flexDirection: "column",
    gap: "2rem",
  },
});

export { styles as transcriptViewerStyles };
