import * as stylex from "@stylexjs/stylex";
import { Fragment, memo, useCallback, useMemo } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

import { SegmentHeader } from "./segment-header";
import {
  getTranscriptSectionKey,
  getTranscriptSegmentDomId,
} from "./selection";
import { useTranscriptSelectionState } from "./selection-context";
import {
  getActiveLineIndex,
  groupWordsIntoLines,
  type HighlightSegment,
} from "./utils";
import { WordSpan } from "./word-span";

import { createHighlightSegments } from "~/session/components/note-input/search/matching";
import type { Segment, SegmentWord } from "~/stt/live-segment";
import { updateTranscriptSegmentText } from "~/stt/queries";

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
    sessionId,
    speakerLabel,
    currentMs,
    seekAndPlay,
    audioExists,
    search,
    editMode = false,
  }: {
    segment: Segment;
    offsetMs: number;
    transcriptId: string;
    sessionId?: string;
    speakerLabel: string;
    currentMs: number;
    seekAndPlay: (word: SegmentWord) => void;
    audioExists: boolean;
    search: TranscriptSearchRenderState;
    editMode?: boolean;
  }) => {
    const lines = useMemo(
      () => groupWordsIntoLines(segment.words),
      [segment.words],
    );
    const { selectMode, isSelected } = useTranscriptSelectionState();
    const segmentId = getTranscriptSegmentDomId(transcriptId, segment);
    const selected = isSelected(
      getTranscriptSectionKey(transcriptId, segmentId),
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
      <section
        data-transcript-id={transcriptId}
        data-transcript-segment-id={segmentId}
        data-session-id={sessionId}
        data-segment-channel={segment.key.channel}
        data-segment-speaker-index={segment.key.speaker_index ?? ""}
        data-segment-speaker-human-id={segment.key.speaker_human_id ?? ""}
        data-transcript-offset-ms={offsetMs}
        data-transcript-selected={selected ? "true" : undefined}
        {...stylex.props(styles.segment, selectMode && styles.selectable)}
      >
        <SegmentHeader
          segment={segment}
          transcriptId={transcriptId}
          sessionId={sessionId}
          label={speakerLabel}
          selected={selected}
        />

        {editMode ? (
          <EditableSegmentText segment={segment} transcriptId={transcriptId} />
        ) : (
          <div
            data-transcript-segment-content
            {...stylex.props(
              styles.segmentContent,
              selectMode ? styles.unselectable : styles.selectableText,
            )}
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
                  {...stylex.props(styles.line)}
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
        )}
      </section>
    );
  },
  (prev, next) => {
    if (
      prev.segment !== next.segment ||
      prev.offsetMs !== next.offsetMs ||
      prev.transcriptId !== next.transcriptId ||
      prev.sessionId !== next.sessionId ||
      prev.speakerLabel !== next.speakerLabel ||
      prev.audioExists !== next.audioExists ||
      prev.seekAndPlay !== next.seekAndPlay ||
      prev.editMode !== next.editMode
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

const EditableSegmentText = memo(function EditableSegmentText({
  segment,
  transcriptId,
}: {
  segment: Segment;
  transcriptId: string;
}) {
  const originalText = normalizeEditableTranscriptText(
    segment.words.map(getWordDisplayText).join(" "),
  );
  const wordIds = useMemo(
    () =>
      segment.words.flatMap((word) =>
        typeof word.id === "string" && word.id ? [word.id] : [],
      ),
    [segment.words],
  );
  const handleBlur = useCallback(
    (event: React.FocusEvent<HTMLDivElement>) => {
      const nextText = normalizeEditableTranscriptText(
        event.currentTarget.innerText,
      );
      if (nextText === originalText || wordIds.length === 0) {
        return;
      }

      void updateTranscriptSegmentText({
        transcriptId,
        wordIds,
        text: nextText,
      }).catch((error) => {
        console.error("[transcript] failed to update text", error);
      });
    },
    [originalText, transcriptId, wordIds],
  );
  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.currentTarget.textContent = originalText;
        event.currentTarget.blur();
        return;
      }

      if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        event.currentTarget.blur();
      }
    },
    [originalText],
  );

  return (
    <div
      data-transcript-segment-content
      data-transcript-editor
      data-transcript-edit-word-ids={JSON.stringify(wordIds)}
      data-transcript-edit-word-start-ms={JSON.stringify(
        segment.words.map((word) => word.start_ms),
      )}
      data-transcript-edit-word-texts={JSON.stringify(
        segment.words.map(getWordDisplayText),
      )}
      contentEditable
      suppressContentEditableWarning
      spellCheck
      {...stylex.props(styles.editor)}
      onBlur={handleBlur}
      onKeyDown={handleKeyDown}
    >
      {originalText}
    </div>
  );
});

export function normalizeEditableTranscriptText(text: string) {
  return text.replace(/\s+/g, " ").trim();
}

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

const styles = stylex.create({
  editor: {
    borderRadius: radii.md,
    fontSize: "0.875rem",
    lineHeight: 1.625,
    marginTop: "0.375rem",
    outline: "none",
    overflowWrap: "anywhere",
    userSelect: {
      default: "text",
      ":is(*) *": "text",
    },
    wordBreak: "break-word",
  },
  line: {
    backgroundColor: {
      default: null,
      ":is([data-line-current='true'])": "rgb(254 249 195 / 0.5)",
      ":is(.dark [data-line-current='true'])": "rgb(113 63 18 / 0.3)",
    },
    borderRadius: "0.125rem",
    marginInline: "-0.125rem",
    paddingInline: "0.125rem",
  },
  segment: {
    backgroundColor: {
      default: null,
      ":is([data-transcript-selected='true'])": `color-mix(in oklab, ${colors.primary} 10%, transparent)`,
    },
    borderRadius: radii.lg,
    boxShadow: {
      default: null,
      ":is([data-transcript-selected='true'])": `inset 0 0 0 1px color-mix(in oklab, ${colors.primary} 30%, transparent)`,
    },
    paddingInline: "0.5rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
  },
  segmentContent: {
    fontSize: "0.875rem",
    lineHeight: 1.625,
    marginTop: "0.375rem",
    overflowWrap: "anywhere",
    wordBreak: "break-word",
  },
  selectable: {
    cursor: "pointer",
  },
  selectableText: {
    userSelect: {
      default: "text",
      ":is(*) *": "text",
    },
  },
  unselectable: {
    userSelect: "none",
  },
});

export { styles as transcriptSegmentStyles };
