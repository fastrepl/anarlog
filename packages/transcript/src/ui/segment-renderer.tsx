import { memo, useCallback, useMemo } from "react";

import { cn } from "@hypr/utils";

import type { Operations, Segment, SegmentWord } from "../shared";
import { SegmentHeader } from "./segment-header";
import type {
  AudioState,
  HighlightSegment,
  Participant,
  SpeakerInfo,
} from "./utils";
import { groupWordsIntoLines } from "./utils";
import { WordSpan } from "./word-span";

export const SegmentRenderer = memo(
  ({
    editable,
    segment,
    offsetMs,
    operations,
    speakerInfo,
    timestamp,
    participants,
    audioState,
    getSearchHighlights,
    onWordContextMenu,
  }: {
    editable: boolean;
    segment: Segment;
    offsetMs: number;
    operations?: Operations;
    speakerInfo: SpeakerInfo;
    timestamp: string;
    participants?: Participant[];
    audioState?: AudioState;
    getSearchHighlights?: (
      word: SegmentWord,
    ) => { segments: HighlightSegment[]; isActive: boolean } | undefined;
    onWordContextMenu?: (word: SegmentWord, event: React.MouseEvent) => void;
  }) => {
    const audioExists = audioState?.exists ?? false;
    const currentMs = audioState?.currentMs ?? 0;

    const seekAndPlay = useCallback(
      (word: SegmentWord) => {
        if (audioState?.exists) {
          audioState.onSeek(word);
        }
      },
      [audioState],
    );

    const lines = useMemo(
      () => groupWordsIntoLines(segment.words),
      [segment.words],
    );

    return (
      <section>
        <SegmentHeader
          segment={segment}
          speakerInfo={speakerInfo}
          timestamp={timestamp}
          operations={operations}
          participants={participants}
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
                    searchHighlights={getSearchHighlights?.(word)}
                    onClickWord={seekAndPlay}
                    onContextMenu={onWordContextMenu}
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
    return (
      prev.editable === next.editable &&
      prev.segment === next.segment &&
      prev.offsetMs === next.offsetMs &&
      prev.operations === next.operations &&
      prev.speakerInfo === next.speakerInfo &&
      prev.timestamp === next.timestamp &&
      prev.participants === next.participants &&
      prev.audioState === next.audioState &&
      prev.getSearchHighlights === next.getSearchHighlights &&
      prev.onWordContextMenu === next.onWordContextMenu
    );
  },
);
