import { ArrowDown, ArrowUp } from "@phosphor-icons/react";
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

import { cn } from "@anlg/utils";

import {
  getTranscriptContextSelection,
  getTranscriptSectionSelection,
  mergeTranscriptSelections,
  type TranscriptWordSelection,
} from "./selection";
import { MultiSelectionBar, SelectionMenu } from "./selection-menu";
import type { TranscriptContextMenuRequest } from "./selection-menu";
import { TranscriptSeparator } from "./separator";
import { RenderTranscript } from "./transcript";
import {
  useAutoScroll,
  usePlaybackAutoScroll,
  useScrollDetection,
} from "./viewport-hooks";

import { trackAnalyticsEvent } from "~/analytics";
import { useAudioPlayer } from "~/audio-player";
import { useAudioTime } from "~/audio-player/provider";
import type { Segment } from "~/stt/live-segment";
import { assignTranscriptSpeaker } from "~/stt/queries";

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
    { enableOnFormTags: false },
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
      await Promise.all(
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
  const handleContextMenu = useCallback(
    (event: ReactMouseEvent<HTMLDivElement>) => {
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
    [],
  );
  const handleContextClose = useCallback(() => {
    setContextRequest(null);
  }, []);
  const clearSelectedEntries = useCallback(() => {
    containerRef.current
      ?.querySelectorAll<HTMLElement>("[data-transcript-selected='true']")
      .forEach((element) => delete element.dataset.transcriptSelected);
    setSelectedEntries(new Map());
    setSelectionAnchor(null);
  }, []);
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

      const hasSelectionModifier =
        event.metaKey || event.ctrlKey || event.shiftKey;
      if (!hasSelectionModifier) {
        if (selectedEntries.size > 0) {
          clearSelectedEntries();
        }
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      window.getSelection()?.removeAllRanges();
      const container = event.currentTarget;
      const sections = [
        ...container.querySelectorAll<HTMLElement>(
          "section[data-transcript-segment-id]",
        ),
      ];
      const targetKey = getTranscriptSectionKey(section);
      if (!targetKey) {
        return;
      }

      setSelectedEntries((current) => {
        const next = new Map(current);
        if (event.shiftKey && selectionAnchor) {
          const anchorIndex = sections.findIndex(
            (candidate) =>
              getTranscriptSectionKey(candidate) === selectionAnchor,
          );
          const targetIndex = sections.indexOf(section);
          if (anchorIndex !== -1 && targetIndex !== -1) {
            const start = Math.min(anchorIndex, targetIndex);
            const end = Math.max(anchorIndex, targetIndex);
            for (const candidate of sections.slice(start, end + 1)) {
              const key = getTranscriptSectionKey(candidate);
              const selection = getTranscriptSectionSelection(
                candidate,
                container,
              );
              if (key && selection) {
                candidate.dataset.transcriptSelected = "true";
                next.set(key, selection);
              }
            }
            return next;
          }
        }

        if (next.has(targetKey)) {
          delete section.dataset.transcriptSelected;
          next.delete(targetKey);
        } else {
          const selection = getTranscriptSectionSelection(section, container);
          if (selection) {
            section.dataset.transcriptSelected = "true";
            next.set(targetKey, selection);
          }
        }
        return next;
      });
      setSelectionAnchor(targetKey);
    },
    [clearSelectedEntries, selectedEntries.size, selectionAnchor],
  );

  return (
    <div className="relative h-full">
      <div
        ref={handleContainerRef}
        data-transcript-container
        onClickCapture={handleSegmentSelection}
        onContextMenu={handleContextMenu}
        className={cn([
          "flex h-full flex-col gap-8 overflow-x-hidden overflow-y-auto",
          "scrollbar-hide",
          "scroll-pb-[calc(8rem+env(safe-area-inset-bottom))]",
          "pb-[calc(4rem+env(safe-area-inset-bottom))]",
        ])}
      >
        {visibleTranscriptIds.map((transcriptId, index) => {
          const isLastTranscript = index === visibleTranscriptIds.length - 1;
          const isActiveTranscript = currentActive && isLastTranscript;

          return (
            <div key={transcriptId} className="flex flex-col gap-8">
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
          onContextClose={handleContextClose}
          onAction={handleSelectionAction}
          onAssignSpeaker={handleAssignSpeaker}
        />
      </div>

      {multiSelection && (
        <MultiSelectionBar
          selection={multiSelection}
          entryCount={selectedEntries.size}
          onClear={clearSelectedEntries}
          onAssignSpeaker={handleAssignSpeaker}
        />
      )}

      {canScroll && (
        <div
          data-transcript-scroll-controls
          className={cn([
            "absolute top-1/2 right-1 z-40 flex -translate-y-1/2 flex-col overflow-hidden",
            "border-border/60 bg-muted/70 text-foreground rounded-full border",
          ])}
        >
          <button
            type="button"
            aria-label="Scroll to top"
            onClick={scrollToTop}
            disabled={isAtTop}
            className={cn([
              "flex size-8 items-center justify-center",
              "hover:bg-muted/85 active:bg-muted/85",
              "disabled:pointer-events-none disabled:opacity-30",
            ])}
          >
            <ArrowUp aria-hidden="true" className="size-3.5" />
          </button>
          <div className="bg-border/70 h-px w-full" />
          <button
            type="button"
            aria-label="Scroll to bottom"
            onClick={scrollToBottom}
            disabled={isAtBottom}
            className={cn([
              "flex size-8 items-center justify-center",
              "hover:bg-muted/85 active:bg-muted/85",
              "disabled:pointer-events-none disabled:opacity-30",
            ])}
          >
            <ArrowDown aria-hidden="true" className="size-3.5" />
          </button>
        </div>
      )}
    </div>
  );
}

function getTranscriptSectionKey(section: HTMLElement) {
  const transcriptId = section.dataset.transcriptId;
  const segmentId = section.dataset.transcriptSegmentId;
  return transcriptId && segmentId ? `${transcriptId}:${segmentId}` : null;
}
