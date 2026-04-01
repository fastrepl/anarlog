import { Fragment, useMemo } from "react";

import { cn } from "@hypr/utils";

import type { HighlightSegment } from "./utils";

import { useSearch } from "~/session/components/note-input/search/context";
import { createHighlightSegments } from "~/session/components/note-input/search/matching";
import type { SegmentWord } from "~/stt/live-segment";

interface WordSpanProps {
  word: SegmentWord;
  displayText: string;
  audioExists: boolean;
  onClickWord: (word: SegmentWord) => void;
}

export function WordSpan(props: WordSpanProps) {
  const searchHighlights = useTranscriptSearchHighlights(
    props.word,
    props.displayText,
  );
  const highlights = searchHighlights ?? {
    segments: [{ text: props.displayText, isMatch: false }],
    isActive: false,
  };
  const content = useHighlightedContent(
    props.word,
    highlights.segments,
    highlights.isActive,
  );
  const className = useMemo(
    () =>
      cn([
        props.audioExists && "cursor-pointer hover:bg-neutral-200/60",
        !props.word.is_final && ["opacity-60", "italic"],
      ]),
    [props.audioExists, props.word.is_final],
  );

  return (
    <span
      onClick={() => props.onClickWord(props.word)}
      className={className}
      data-word-id={props.word.id}
    >
      {content}
    </span>
  );
}

function useTranscriptSearchHighlights(word: SegmentWord, displayText: string) {
  const search = useSearch();
  const query = search?.query?.trim() ?? "";
  const isVisible = Boolean(search?.isVisible);
  const activeMatchId = search?.activeMatchId ?? null;
  const caseSensitive = search?.caseSensitive ?? false;
  const wholeWord = search?.wholeWord ?? false;

  const segments = useMemo(() => {
    const text = displayText ?? "";
    if (!text) {
      return [{ text: "", isMatch: false }];
    }

    if (!isVisible || !query) {
      return [{ text, isMatch: false }];
    }

    return createHighlightSegments(text, query, caseSensitive, wholeWord);
  }, [caseSensitive, displayText, isVisible, query, wholeWord]);

  return { segments, isActive: word.id === activeMatchId };
}

function useHighlightedContent(
  word: SegmentWord,
  segments: HighlightSegment[],
  isActive: boolean,
) {
  return useMemo(() => {
    const baseKey = word.id ?? word.text ?? "word";

    return segments.map((segment, index) =>
      segment.isMatch ? (
        <span
          key={`${baseKey}-match-${index}`}
          className={isActive ? "bg-yellow-500" : "bg-yellow-200/50"}
        >
          {segment.text}
        </span>
      ) : (
        <Fragment key={`${baseKey}-text-${index}`}>{segment.text}</Fragment>
      ),
    );
  }, [isActive, segments, word.id, word.text]);
}
