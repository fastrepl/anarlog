import { memo, useCallback, useEffect, useMemo } from "react";

import { cn } from "@hypr/utils";

import { useSearch } from "../../search/context";
import { useRenderedTranscriptData, useTranscriptOffset } from "./data-hooks";
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
  applyRenderRequestIdentitiesToSegments,
  getMaxSpeakerNumberForParticipants,
  mergeRenderedAndLiveSegments,
  SegmentKeyUtils,
  type RenderLabelContext,
  type Segment,
  type SegmentWord,
} from "~/stt/live-segment";
import { useTranscript, useTranscriptLabelContext } from "~/stt/queries";
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
      shouldScrollToEnd={shouldScrollToEnd}
      currentMs={currentMs}
      seek={seek}
      startPlayback={startPlayback}
      audioExists={audioExists}
      maxSpeakerNumber={maxSpeakerNumber}
    />
  );
}

function TranscriptSegments({
  segments: rawSegments,
  scrollElement,
  transcriptId,
  shouldScrollToEnd,
  currentMs,
  seek,
  startPlayback,
  audioExists,
  maxSpeakerNumber,
}: {
  segments: Segment[];
  scrollElement: HTMLDivElement | null;
  transcriptId: string;
  shouldScrollToEnd: boolean;
  currentMs: number;
  seek: (sec: number) => void;
  startPlayback: () => void;
  audioExists: boolean;
  maxSpeakerNumber?: number;
}) {
  const segments = useStableSegments(rawSegments);
  const offsetMs = useTranscriptOffset(transcriptId);
  const transcript = useTranscript(transcriptId);
  const labelContext = useTranscriptLabelContext(transcriptId);

  if (segments.length === 0) {
    return null;
  }

  return (
    <SegmentsList
      segments={segments}
      scrollElement={scrollElement}
      transcriptId={transcriptId}
      sessionId={transcript?.sessionId}
      labelContext={labelContext}
      offsetMs={offsetMs}
      shouldScrollToEnd={shouldScrollToEnd}
      currentMs={currentMs}
      seek={seek}
      startPlayback={startPlayback}
      audioExists={audioExists}
      maxSpeakerNumber={maxSpeakerNumber}
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

    const seekAndPlay = useCallback(
      (word: SegmentWord) => {
        if (audioExists && isTranscriptWordSeekable(word)) {
          seek((offsetMs + word.start_ms) / 1000);
          startPlayback();
        }
      },
      [audioExists, offsetMs, seek, startPlayback],
    );

    useEffect(() => {
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
      <div>
        {segments.map((segment, index) => (
          <div
            key={createSegmentKey(segment, transcriptId, index)}
            className={cn([index > 0 && "pt-4"])}
          >
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
            />
          </div>
        ))}
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
      prevProps.seek === nextProps.seek &&
      prevProps.startPlayback === nextProps.startPlayback &&
      segmentsShallowEqual(prevProps.segments, nextProps.segments)
    );
  },
);
