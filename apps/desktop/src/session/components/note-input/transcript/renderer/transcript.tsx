import { memo, useCallback, useEffect, useMemo } from "react";

import { cn } from "@hypr/utils";

import {
  useRenderedTranscriptSegments,
  useTranscriptOffset,
} from "./data-hooks";
import { SegmentRenderer } from "./segment";
import {
  createSegmentKey,
  segmentsShallowEqual,
  useStableSegments,
} from "./segment-hooks";

import * as main from "~/store/tinybase/store/main";
import type { Segment, SegmentWord } from "~/stt/live-segment";
import {
  defaultRenderLabelContext,
  SpeakerLabelManager,
} from "~/stt/segment/shared";

export function RenderTranscript({
  scrollElement,
  isLastTranscript,
  isAtBottom,
  transcriptId,
  liveSegments,
  currentMs,
  seek,
  startPlayback,
  audioExists,
}: {
  scrollElement: HTMLDivElement | null;
  isLastTranscript: boolean;
  isAtBottom: boolean;
  transcriptId: string;
  liveSegments: Segment[];
  currentMs: number;
  seek: (sec: number) => void;
  startPlayback: () => void;
  audioExists: boolean;
}) {
  const storedSegments = useRenderedTranscriptSegments(transcriptId);
  const segments = useStableSegments(
    liveSegments.length > 0 ? liveSegments : storedSegments,
  );
  const offsetMs = useTranscriptOffset(transcriptId);

  if (segments.length === 0) {
    return null;
  }

  return (
    <SegmentsList
      segments={segments}
      scrollElement={scrollElement}
      transcriptId={transcriptId}
      offsetMs={offsetMs}
      shouldScrollToEnd={isLastTranscript && isAtBottom}
      currentMs={currentMs}
      seek={seek}
      startPlayback={startPlayback}
      audioExists={audioExists}
    />
  );
}

const SegmentsList = memo(
  ({
    segments,
    scrollElement,
    transcriptId,
    offsetMs,
    shouldScrollToEnd,
    currentMs,
    seek,
    startPlayback,
    audioExists,
  }: {
    segments: Segment[];
    scrollElement: HTMLDivElement | null;
    transcriptId: string;
    offsetMs: number;
    shouldScrollToEnd: boolean;
    currentMs: number;
    seek: (sec: number) => void;
    startPlayback: () => void;
    audioExists: boolean;
  }) => {
    const store = main.UI.useStore(main.STORE_ID);
    const speakerLabelManager = useMemo(() => {
      if (!store) {
        return new SpeakerLabelManager();
      }
      const ctx = defaultRenderLabelContext(store);
      return SpeakerLabelManager.fromSegments(segments, ctx);
    }, [segments, store]);

    const seekAndPlay = useCallback(
      (word: SegmentWord) => {
        if (audioExists) {
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
            className={cn([index > 0 && "pt-8"])}
          >
            <SegmentRenderer
              segment={segment}
              offsetMs={offsetMs}
              transcriptId={transcriptId}
              speakerLabelManager={speakerLabelManager}
              currentMs={currentMs}
              seekAndPlay={seekAndPlay}
              audioExists={audioExists}
            />
          </div>
        ))}
      </div>
    );
  },
  (prevProps, nextProps) => {
    return (
      prevProps.transcriptId === nextProps.transcriptId &&
      prevProps.scrollElement === nextProps.scrollElement &&
      prevProps.offsetMs === nextProps.offsetMs &&
      prevProps.shouldScrollToEnd === nextProps.shouldScrollToEnd &&
      prevProps.currentMs === nextProps.currentMs &&
      prevProps.audioExists === nextProps.audioExists &&
      prevProps.seek === nextProps.seek &&
      prevProps.startPlayback === nextProps.startPlayback &&
      segmentsShallowEqual(prevProps.segments, nextProps.segments)
    );
  },
);
