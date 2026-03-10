import { memo, useMemo } from "react";

import type { Operations, Segment, SegmentWord } from "@hypr/transcript";
import { SpeakerLabelManager } from "@hypr/transcript";
import { groupWordsIntoLines } from "@hypr/transcript/ui";
import { cn } from "@hypr/utils";

import { SegmentHeader } from "./segment-header";
import { WordSpan } from "./word-span";

function getSegmentTimeRange(
  segment: Segment,
  offsetMs: number,
): { start: number; end: number } | null {
  const words = segment.words;
  if (words.length === 0) return null;
  return {
    start: offsetMs + (words[0].start_ms ?? 0),
    end: offsetMs + (words[words.length - 1].end_ms ?? 0),
  };
}

export const SegmentRenderer = memo(
  ({
    editable,
    segment,
    offsetMs,
    operations,
    sessionId,
    speakerLabelManager,
    currentMs,
    seekAndPlay,
    audioExists,
  }: {
    editable: boolean;
    segment: Segment;
    offsetMs: number;
    operations?: Operations;
    sessionId?: string;
    speakerLabelManager?: SpeakerLabelManager;
    currentMs: number;
    seekAndPlay: (word: SegmentWord) => void;
    audioExists: boolean;
  }) => {
    const lines = useMemo(
      () => groupWordsIntoLines(segment.words),
      [segment.words],
    );

    return (
      <section>
        <SegmentHeader
          segment={segment}
          operations={operations}
          sessionId={sessionId}
          speakerLabelManager={speakerLabelManager}
        />

        <div
          className={cn([
            "overflow-wrap-anywhere mt-1.5 text-sm leading-relaxed wrap-break-word",
            editable && "select-text-deep",
          ])}
        >
          {lines.map((line, lineIdx) => {
            const lineStartMs = offsetMs + line.startMs;
            const lineEndMs = offsetMs + line.endMs;
            const isCurrentLine =
              audioExists &&
              currentMs > 0 &&
              currentMs >= lineStartMs &&
              currentMs <= lineEndMs;

            return (
              <span
                key={line.words[0]?.id ?? `line-${lineIdx}`}
                data-line-current={isCurrentLine ? "true" : undefined}
                className={cn([
                  "-mx-0.5 rounded-xs px-0.5",
                  isCurrentLine && "bg-yellow-100/50",
                ])}
              >
                {line.words.map((word, idx) => (
                  <WordSpan
                    key={word.id ?? `${word.start_ms}-${idx}`}
                    word={word}
                    audioExists={audioExists}
                    operations={operations}
                    onClickWord={seekAndPlay}
                  />
                ))}
              </span>
            );
          })}
        </div>
      </section>
    );
  },
  (prev, next) => {
    if (
      prev.editable !== next.editable ||
      prev.segment !== next.segment ||
      prev.offsetMs !== next.offsetMs ||
      prev.operations !== next.operations ||
      prev.sessionId !== next.sessionId ||
      prev.speakerLabelManager !== next.speakerLabelManager ||
      prev.audioExists !== next.audioExists ||
      prev.seekAndPlay !== next.seekAndPlay
    ) {
      return false;
    }

    // Smart time comparison: only re-render if time change affects which line is active
    if (prev.currentMs === next.currentMs) return true;

    const range = getSegmentTimeRange(prev.segment, prev.offsetMs);
    if (!range) return true;

    const prevInRange =
      prev.currentMs > 0 &&
      prev.currentMs >= range.start &&
      prev.currentMs <= range.end;
    const nextInRange =
      next.currentMs > 0 &&
      next.currentMs >= range.start &&
      next.currentMs <= range.end;

    // If neither time is in this segment's range, no visual change
    if (!prevInRange && !nextInRange) return true;

    // If both are in range, the active line might have changed — need to re-render
    // If one is in range and the other isn't, definitely need to re-render
    return false;
  },
);
