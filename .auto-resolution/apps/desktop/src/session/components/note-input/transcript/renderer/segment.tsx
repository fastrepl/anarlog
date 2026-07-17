import { Fragment, memo, useMemo } from "react";

import { cn } from "@hypr/utils";

import { SegmentHeader } from "./segment-header";
import {
  getActiveLineIndex,
  groupWordsIntoLines,
  type HighlightSegment,
} from "./utils";
import { WordSpan } from "./word-span";

import { createHighlightSegments } from "~/session/components/note-input/search/matching";
import type { Segment, SegmentWord } from "~/stt/live-segment";
import { SpeakerLabelManager } from "~/stt/live-segment";

export type TranscriptSearchRenderState = {
  query: string;
  activeMatchId: string | null;
  caseSensitive: boolean;
  wholeWord: boolean;
};

export const EMPTY_TRANSCRIPT_SEARCH: TranscriptSearchRenderState = {
  query: "",
  activeMatchId: null,
  caseSensitive: false,
  wholeWord: false,
};

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
    segment,
    offsetMs,
    transcriptId,
    speakerLabelManager,
    currentMs,
    seekAndPlay,
    audioExists,
    search,
  }: {
    segment: Segment;
    offsetMs: number;
    transcriptId: string;
    speakerLabelManager?: SpeakerLabelManager;
    currentMs: number;
    seekAndPlay: (word: SegmentWord) => void;
    audioExists: boolean;
    search: TranscriptSearchRenderState;
  }) => {
    const lines = useMemo(
      () => groupWordsIntoLines(segment.words),
      [segment.words],
    );
    const highlightSegmentsByWord = useMemo(() => {
      if (!search.query) {
        return null;
      }

      const highlights = new Map<SegmentWord, HighlightSegment[]>();
      for (const word of segment.words) {
        const displayText = getWordDisplayText(word);
        highlights.set(
          word,
          createHighlightSegments(
            displayText,
            search.query,
            search.caseSensitive,
            search.wholeWord,
          ),
        );
      }
      return highlights;
    }, [search.caseSensitive, search.query, search.wholeWord, segment.words]);

    return (
      <section>
        <SegmentHeader
          segment={segment}
          transcriptId={transcriptId}
          speakerLabelManager={speakerLabelManager}
        />

        <div
          className={cn([
            "overflow-wrap-anywhere mt-1.5 text-sm leading-relaxed wrap-break-word",
            "select-text-deep",
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
                  isCurrentLine && "bg-yellow-100/50 dark:bg-yellow-900/30",
                ])}
              >
                {lineIdx > 0 ? " " : null}
                {line.words.map((word, idx) => (
                  <Fragment key={word.id ?? `${word.start_ms}-${idx}`}>
                    {idx > 0 ? " " : null}
                    <WordSpan
                      word={word}
                      displayText={getWordDisplayText(word)}
                      audioExists={audioExists}
                      onClickWord={seekAndPlay}
                      highlightSegments={
                        highlightSegmentsByWord?.get(word) ?? undefined
                      }
                      isActiveMatch={
                        Boolean(word.id) && word.id === search.activeMatchId
                      }
                    />
                  </Fragment>
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
      prev.segment !== next.segment ||
      prev.offsetMs !== next.offsetMs ||
      prev.transcriptId !== next.transcriptId ||
      prev.speakerLabelManager !== next.speakerLabelManager ||
      prev.audioExists !== next.audioExists ||
      prev.seekAndPlay !== next.seekAndPlay
    ) {
      return false;
    }

    if (!canReuseSegmentForSearch(prev, next)) {
      return false;
    }

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

    if (!prevInRange && !nextInRange) return true;

    return (
      getActiveLineIndex(prev.segment.words, prev.offsetMs, prev.currentMs) ===
      getActiveLineIndex(next.segment.words, next.offsetMs, next.currentMs)
    );
  },
);

function canReuseSegmentForSearch(
  prev: { segment: Segment; search: TranscriptSearchRenderState },
  next: { segment: Segment; search: TranscriptSearchRenderState },
) {
  if (
    prev.search.query !== next.search.query ||
    prev.search.caseSensitive !== next.search.caseSensitive ||
    prev.search.wholeWord !== next.search.wholeWord
  ) {
    return false;
  }

  if (prev.search.activeMatchId === next.search.activeMatchId) {
    return true;
  }

  return (
    !segmentContainsWordId(prev.segment, prev.search.activeMatchId) &&
    !segmentContainsWordId(next.segment, next.search.activeMatchId)
  );
}

function segmentContainsWordId(segment: Segment, wordId: string | null) {
  if (!wordId) {
    return false;
  }

  return segment.words.some((word) => word.id === wordId);
}

function getWordDisplayText(word: SegmentWord) {
  return word.text.trim();
}
