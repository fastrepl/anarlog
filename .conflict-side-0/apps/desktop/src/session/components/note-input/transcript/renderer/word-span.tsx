import { Fragment, memo, useMemo } from "react";

import { cn } from "@hypr/utils";

import type { HighlightSegment } from "./utils";

import type { SegmentWord } from "~/stt/live-segment";
import { isTranscriptWordSeekable } from "~/stt/timing";

interface WordSpanProps {
  word: SegmentWord;
  displayText: string;
  audioExists: boolean;
  onClickWord: (word: SegmentWord) => void;
  highlightSegments?: HighlightSegment[];
  isActiveMatch?: boolean;
}

export const WordSpan = memo(function WordSpan(props: WordSpanProps) {
  const content = useHighlightedContent(
    props.word,
    props.displayText,
    props.highlightSegments,
    props.isActiveMatch ?? false,
  );
  const canSeek = props.audioExists && isTranscriptWordSeekable(props.word);
  const className = useMemo(
    () =>
      cn([
        canSeek && "hover:bg-accent/60 cursor-pointer",
        !props.word.is_final && ["opacity-60", "italic"],
      ]),
    [canSeek, props.word.is_final],
  );

  return (
    <span
      onClick={() => canSeek && props.onClickWord(props.word)}
      className={className}
      data-word-id={props.word.id}
    >
      {content}
    </span>
  );
});

function useHighlightedContent(
  word: SegmentWord,
  displayText: string,
  segments: HighlightSegment[] | undefined,
  isActive: boolean,
) {
  return useMemo(() => {
    if (!segments) {
      return displayText;
    }

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
  }, [displayText, isActive, segments, word.id, word.text]);
}
