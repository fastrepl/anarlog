import {
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
} from "react";

import type { RenderTranscriptRequest } from "@anlg/plugin-transcription";
import { cn } from "@anlg/utils";

import { useSearch } from "../../search/context";
import {
  useRenderedTranscriptData,
  useTranscriptTimelineMetadata,
} from "./data-hooks";
import {
  EMPTY_TRANSCRIPT_SEARCH,
  SegmentRenderer,
  type TranscriptSearchRenderState,
} from "./segment";
import {
  createSegmentKey,
  segmentsShallowEqual,
  useStableSegments,
} from "./segment-hooks";
import {
  getTranscriptSectionKey,
  getTranscriptSegmentDomId,
  getTranscriptSelectionFromSegment,
  type TranscriptWordSelection,
} from "./selection";
import { useRegisterTranscriptSelectionSource } from "./selection-context";
import { useVirtualSegments, VirtualSegmentRow } from "./virtual-segments";

import {
  applyRenderRequestIdentitiesToSegments,
  getMaxSpeakerNumberForParticipants,
  mergeRenderedAndLiveSegments,
  SegmentKeyUtils,
  type RenderLabelContext,
  type Segment,
  type SegmentWord,
} from "~/stt/live-segment";
import { SpeakerLabelManager } from "~/stt/segment/shared";
import { isTranscriptWordSeekable } from "~/stt/timing";

export function RenderTranscript({
  scrollElement,
  isLastTranscript,
  shouldScrollToEnd,
  transcriptId,
  currentActive,
  captureGeneration = 0,
  liveSegments,
  currentMs,
  seek,
  startPlayback,
  audioExists,
  editMode = false,
}: {
  scrollElement: HTMLDivElement | null;
  isLastTranscript: boolean;
  shouldScrollToEnd: boolean;
  transcriptId: string;
  currentActive: boolean;
  captureGeneration?: number;
  liveSegments: Segment[];
  currentMs: number;
  seek: (sec: number) => void;
  startPlayback: () => void;
  audioExists: boolean;
  editMode?: boolean;
}) {
  return (
    <PersistedTranscript
      scrollElement={scrollElement}
      transcriptId={transcriptId}
      currentActive={currentActive}
      captureGeneration={captureGeneration}
      liveSegments={liveSegments}
      shouldScrollToEnd={isLastTranscript && shouldScrollToEnd}
      currentMs={currentMs}
      seek={seek}
      startPlayback={startPlayback}
      audioExists={audioExists}
      editMode={editMode}
    />
  );
}

function PersistedTranscript({
  scrollElement,
  transcriptId,
  currentActive,
  captureGeneration,
  liveSegments,
  shouldScrollToEnd,
  currentMs,
  seek,
  startPlayback,
  audioExists,
  editMode,
}: {
  scrollElement: HTMLDivElement | null;
  transcriptId: string;
  currentActive: boolean;
  captureGeneration: number;
  liveSegments: Segment[];
  shouldScrollToEnd: boolean;
  currentMs: number;
  seek: (sec: number) => void;
  startPlayback: () => void;
  audioExists: boolean;
  editMode: boolean;
}) {
  const {
    maxSpeakerNumber,
    request,
    segments: storedSegments,
  } = useRenderedTranscriptData(transcriptId, currentActive, captureGeneration);
  const mergedSegments = useMemo(() => {
    const merged = mergeRenderedAndLiveSegments(
      storedSegments,
      liveSegments,
      currentActive ? request : null,
    );
    return currentActive
      ? applyRenderRequestIdentitiesToSegments(merged, request)
      : merged;
  }, [currentActive, liveSegments, request, storedSegments]);

  return (
    <TranscriptSegments
      segments={mergedSegments}
      scrollElement={scrollElement}
      transcriptId={transcriptId}
      currentActive={currentActive}
      shouldScrollToEnd={shouldScrollToEnd}
      currentMs={currentMs}
      seek={seek}
      startPlayback={startPlayback}
      audioExists={audioExists}
      maxSpeakerNumber={maxSpeakerNumber}
      request={request}
      editMode={editMode}
    />
  );
}

function TranscriptSegments({
  segments: rawSegments,
  scrollElement,
  transcriptId,
  currentActive,
  shouldScrollToEnd,
  currentMs,
  seek,
  startPlayback,
  audioExists,
  maxSpeakerNumber,
  request,
  editMode,
}: {
  segments: Segment[];
  scrollElement: HTMLDivElement | null;
  transcriptId: string;
  currentActive: boolean;
  shouldScrollToEnd: boolean;
  currentMs: number;
  seek: (sec: number) => void;
  startPlayback: () => void;
  audioExists: boolean;
  maxSpeakerNumber?: number;
  request: RenderTranscriptRequest | null;
  editMode: boolean;
}) {
  const segments = useStableSegments(rawSegments);
  const { offsetMs, sessionId } = useTranscriptTimelineMetadata(
    transcriptId,
    !currentActive,
  );
  const labelContext = useMemo<RenderLabelContext | undefined>(() => {
    if (!request) return undefined;

    const names = new Map(
      request.humans.map((human) => [human.human_id, human.name]),
    );
    return {
      getSelfHumanId: () => request.self_human_id ?? undefined,
      getHumanName: (humanId) => names.get(humanId),
      getParticipantHumanIds: () => request.participant_human_ids,
    };
  }, [request]);

  if (segments.length === 0) {
    return null;
  }

  return (
    <SegmentsList
      segments={segments}
      scrollElement={scrollElement}
      transcriptId={transcriptId}
      sessionId={sessionId}
      labelContext={labelContext}
      offsetMs={offsetMs}
      shouldScrollToEnd={shouldScrollToEnd}
      currentMs={currentMs}
      seek={seek}
      startPlayback={startPlayback}
      audioExists={audioExists}
      maxSpeakerNumber={maxSpeakerNumber}
      editMode={editMode}
    />
  );
}

const SegmentsList = memo(
  ({
    segments,
    scrollElement,
    transcriptId,
    sessionId,
    labelContext,
    offsetMs,
    shouldScrollToEnd,
    currentMs,
    seek,
    startPlayback,
    audioExists,
    maxSpeakerNumber,
    editMode,
  }: {
    segments: Segment[];
    scrollElement: HTMLDivElement | null;
    transcriptId: string;
    sessionId?: string;
    labelContext?: RenderLabelContext;
    offsetMs: number;
    shouldScrollToEnd: boolean;
    currentMs: number;
    seek: (sec: number) => void;
    startPlayback: () => void;
    audioExists: boolean;
    maxSpeakerNumber?: number;
    editMode: boolean;
  }) => {
    const search = useSearch();
    const inferredMaxSpeakerNumber = labelContext
      ? getMaxSpeakerNumberForParticipants(
          labelContext.getParticipantHumanIds?.() ?? [],
          labelContext.getSelfHumanId(),
        )
      : undefined;
    const resolvedMaxSpeakerNumber =
      maxSpeakerNumber ?? inferredMaxSpeakerNumber;
    const speakerLabelManager = useMemo(() => {
      return labelContext
        ? SpeakerLabelManager.fromSegments(
            segments,
            labelContext,
            resolvedMaxSpeakerNumber,
          )
        : new SpeakerLabelManager(resolvedMaxSpeakerNumber);
    }, [labelContext, resolvedMaxSpeakerNumber, segments]);
    const speakerLabels = useMemo(() => {
      const labels = new Map<Segment, string>();
      for (const segment of segments) {
        labels.set(
          segment,
          SegmentKeyUtils.renderLabel(
            segment.key,
            labelContext,
            speakerLabelManager,
          ),
        );
      }
      return labels;
    }, [labelContext, segments, speakerLabelManager]);
    const transcriptSearch = useMemo<TranscriptSearchRenderState>(() => {
      const query = search?.query.trim() ?? "";
      if (!search?.isVisible || !query) {
        return EMPTY_TRANSCRIPT_SEARCH;
      }

      return {
        query,
        activeMatchId: search.activeMatchId,
        caseSensitive: search.caseSensitive,
        wholeWord: search.wholeWord,
      };
    }, [
      search?.activeMatchId,
      search?.caseSensitive,
      search?.isVisible,
      search?.query,
      search?.wholeWord,
    ]);
    const segmentKeys = useMemo(
      () =>
        segments.map((segment, index) =>
          createSegmentKey(segment, transcriptId, index),
        ),
      [segments, transcriptId],
    );
    const virtual = useVirtualSegments({
      segments,
      segmentKeys,
      scrollElement,
      activeMatchId: transcriptSearch.activeMatchId,
      searchEnabled: transcriptSearch.query.length > 0,
      currentMs,
      offsetMs,
    });
    useRegisterTranscriptSegments(transcriptId, sessionId, offsetMs, segments);

    const seekAndPlay = useCallback(
      (word: SegmentWord) => {
        if (audioExists && isTranscriptWordSeekable(word)) {
          seek((offsetMs + word.start_ms) / 1000);
          startPlayback();
        }
      },
      [audioExists, offsetMs, seek, startPlayback],
    );

    useLayoutEffect(() => {
      if (!scrollElement || !shouldScrollToEnd) {
        return;
      }
      const raf = requestAnimationFrame(() => {
        scrollElement.scrollTo({
          top: scrollElement.scrollHeight,
          behavior: "auto",
        });
      });
      return () => cancelAnimationFrame(raf);
    }, [scrollElement, segments.length, shouldScrollToEnd]);

    return (
      <div
        ref={virtual.listRef}
        data-transcript-virtual-total={segments.length}
        className="relative w-full min-w-0 overflow-x-clip"
        style={{ height: virtual.totalHeight }}
      >
        {virtual.virtualItems.map(({ index, key, top }) => {
          const segment = segments[index]!;
          return (
            <VirtualSegmentRow
              key={key}
              rowKey={key}
              index={index}
              top={top}
              onMeasure={virtual.measureRow}
              onFocus={virtual.handleRowFocus}
              onBlur={virtual.handleRowBlur}
            >
              <div className={cn([index > 0 && "pt-4"])}>
                <SegmentRenderer
                  segment={segment}
                  offsetMs={offsetMs}
                  transcriptId={transcriptId}
                  sessionId={sessionId}
                  speakerLabel={
                    speakerLabels.get(segment) ??
                    SegmentKeyUtils.renderLabel(segment.key)
                  }
                  currentMs={currentMs}
                  seekAndPlay={seekAndPlay}
                  audioExists={audioExists}
                  search={transcriptSearch}
                  editMode={editMode}
                />
              </div>
            </VirtualSegmentRow>
          );
        })}
      </div>
    );
  },
  (prevProps, nextProps) => {
    return (
      prevProps.transcriptId === nextProps.transcriptId &&
      prevProps.sessionId === nextProps.sessionId &&
      prevProps.labelContext === nextProps.labelContext &&
      prevProps.scrollElement === nextProps.scrollElement &&
      prevProps.offsetMs === nextProps.offsetMs &&
      prevProps.shouldScrollToEnd === nextProps.shouldScrollToEnd &&
      prevProps.currentMs === nextProps.currentMs &&
      prevProps.audioExists === nextProps.audioExists &&
      prevProps.maxSpeakerNumber === nextProps.maxSpeakerNumber &&
      prevProps.editMode === nextProps.editMode &&
      prevProps.seek === nextProps.seek &&
      prevProps.startPlayback === nextProps.startPlayback &&
      segmentsShallowEqual(prevProps.segments, nextProps.segments)
    );
  },
);

function useRegisterTranscriptSegments(
  transcriptId: string,
  sessionId: string | undefined,
  offsetMs: number,
  segments: Segment[],
) {
  const registerSource = useRegisterTranscriptSelectionSource();
  const getEntriesRef = useRef<() => Array<[string, TranscriptWordSelection]>>(
    () => [],
  );
  getEntriesRef.current = () =>
    segments.flatMap((segment) => {
      const selection = getTranscriptSelectionFromSegment({
        transcriptId,
        sessionId,
        offsetMs,
        segment,
      });
      if (!selection) {
        return [];
      }
      return [
        [
          getTranscriptSectionKey(
            transcriptId,
            getTranscriptSegmentDomId(transcriptId, segment),
          ),
          selection,
        ] as const,
      ];
    });

  useEffect(() => {
    return registerSource(transcriptId, () => getEntriesRef.current());
  }, [registerSource, transcriptId]);
}
