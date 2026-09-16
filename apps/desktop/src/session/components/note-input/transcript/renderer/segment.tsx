import { Fragment, memo, useCallback, useMemo, useRef, useState } from "react";

import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@anlg/ui/components/ui/popover";
import { cn } from "@anlg/utils";

import { SegmentHeader } from "./segment-header";
import {
  getTranscriptSectionKey,
  getTranscriptSegmentDomId,
} from "./selection";
import { useTranscriptSelectionState } from "./selection-context";
import { SpeakerParticipantPicker } from "./speaker-assign";
import {
  getActiveLineIndex,
  groupWordsIntoLines,
  type HighlightSegment,
} from "./utils";
import { WordSpan } from "./word-span";

import { createHighlightSegments } from "~/session/components/note-input/search/matching";
import type { Segment, SegmentWord } from "~/stt/live-segment";
import {
  splitTranscriptSpeaker,
  updateTranscriptSegmentText,
} from "~/stt/queries";

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
        className={cn([
          "rounded-lg px-2 transition-colors",
          selectMode ? "cursor-pointer" : null,
          "data-[transcript-selected=true]:bg-primary/10 data-[transcript-selected=true]:ring-primary/30 data-[transcript-selected=true]:ring-1 data-[transcript-selected=true]:ring-inset",
        ])}
      >
        <SegmentHeader
          segment={segment}
          transcriptId={transcriptId}
          sessionId={sessionId}
          label={speakerLabel}
          selected={selected}
        />

        {editMode ? (
          <EditableSegmentText
            segment={segment}
            transcriptId={transcriptId}
            sessionId={sessionId}
          />
        ) : (
          <div
            data-transcript-segment-content
            className={cn([
              "overflow-wrap-anywhere mt-1.5 text-sm leading-relaxed wrap-break-word",
              selectMode ? "select-none" : "select-text-deep",
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
  sessionId,
}: {
  segment: Segment;
  transcriptId: string;
  sessionId?: string;
}) {
  const editorRef = useRef<HTMLDivElement>(null);
  const [speakerChange, setSpeakerChange] = useState<{
    text: string;
    offset: number;
  } | null>(null);
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
      if (speakerChange) return;
      const nextText = normalizeEditableTranscriptText(
        event.currentTarget.innerText ?? event.currentTarget.textContent ?? "",
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
    [originalText, speakerChange, transcriptId, wordIds],
  );
  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.currentTarget.textContent = originalText;
        event.currentTarget.blur();
        return;
      }

      if (event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229)
        return;

      if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        event.currentTarget.blur();
        return;
      }
      if (event.key !== "Enter" || event.shiftKey || event.altKey) return;
      const selection = window.getSelection();
      if (!selection?.isCollapsed || !selection.rangeCount) return;
      const range = selection.getRangeAt(0);
      if (!event.currentTarget.contains(range.startContainer)) return;
      const before = range.cloneRange();
      before.selectNodeContents(event.currentTarget);
      before.setEnd(range.startContainer, range.startOffset);
      const text = event.currentTarget.textContent ?? "";
      const offset = before.toString().length;
      event.preventDefault();
      if (text.slice(offset).trim()) setSpeakerChange({ text, offset });
    },
    [originalText],
  );

  return (
    <Popover
      open={speakerChange !== null}
      onOpenChange={(open) => {
        if (!open) setSpeakerChange(null);
      }}
    >
      <div
        ref={editorRef}
        hidden={speakerChange !== null}
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
        className={cn([
          "overflow-wrap-anywhere mt-1.5 rounded-md text-sm leading-relaxed wrap-break-word outline-hidden",
          "select-text-deep",
        ])}
        onBlur={handleBlur}
        onKeyDown={handleKeyDown}
      >
        {originalText}
      </div>
      {speakerChange && (
        <div
          data-transcript-split-preview
          className="mt-1.5 text-sm leading-relaxed wrap-break-word"
        >
          {speakerChange.text.slice(0, speakerChange.offset).trim()}
          <PopoverAnchor asChild>
            <div data-transcript-speaker-split className="mt-5">
              {speakerChange.text.slice(speakerChange.offset).trim()}
            </div>
          </PopoverAnchor>
        </div>
      )}
      <PopoverContent
        variant="app"
        side="bottom"
        align="start"
        collisionPadding={12}
        className="flex max-h-(--radix-popover-content-available-height) w-80 max-w-[calc(100vw-24px)] flex-col overflow-hidden"
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          requestAnimationFrame(() =>
            editorRef.current?.focus({ preventScroll: true }),
          );
        }}
      >
        <SpeakerParticipantPicker
          sessionId={sessionId}
          showAssignmentScope={false}
          onSelect={async (humanId) => {
            if (!speakerChange) return;
            await splitTranscriptSpeaker({
              transcriptId,
              segmentKey: segment.key,
              wordIds,
              ...speakerChange,
              humanId,
            });
            setSpeakerChange(null);
          }}
        />
      </PopoverContent>
    </Popover>
  );
});

function normalizeEditableTranscriptText(text: string) {
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
