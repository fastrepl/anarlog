import { Fragment, useCallback, useMemo } from "react";

import { cn } from "@hypr/utils";

import type { Operations, SegmentWord } from "../shared";
import type { HighlightSegment } from "./utils";

export interface WordSpanProps {
  word: SegmentWord;
  audioExists: boolean;
  operations?: Operations;
  searchHighlights?: { segments: HighlightSegment[]; isActive: boolean };
  onClickWord: (word: SegmentWord) => void;
  onContextMenu?: (word: SegmentWord, event: React.MouseEvent) => void;
}

export function WordSpan(props: WordSpanProps) {
  const hasOperations =
    props.operations && Object.keys(props.operations).length > 0;

  if (hasOperations && props.word.id) {
    return <EditorWordSpan {...props} operations={props.operations!} />;
  }

  return <ViewerWordSpan {...props} />;
}

function ViewerWordSpan({
  word,
  audioExists,
  searchHighlights,
  onClickWord,
}: Omit<WordSpanProps, "operations" | "onContextMenu">) {
  const highlights = searchHighlights ?? {
    segments: [{ text: word.text ?? "", isMatch: false }],
    isActive: false,
  };

  const content = useHighlightedContent(
    word,
    highlights.segments,
    highlights.isActive,
  );

  const className = useMemo(
    () =>
      cn([
        audioExists && "cursor-pointer hover:bg-neutral-200/60",
        !word.isFinal && ["opacity-60", "italic"],
      ]),
    [audioExists, word.isFinal],
  );

  const handleClick = useCallback(() => {
    onClickWord(word);
  }, [word, onClickWord]);

  return (
    <span onClick={handleClick} className={className} data-word-id={word.id}>
      {content}
    </span>
  );
}

function EditorWordSpan({
  word,
  audioExists,
  searchHighlights,
  onClickWord,
  onContextMenu,
}: Omit<WordSpanProps, "operations"> & { operations: Operations }) {
  const highlights = searchHighlights ?? {
    segments: [{ text: word.text ?? "", isMatch: false }],
    isActive: false,
  };

  const content = useHighlightedContent(
    word,
    highlights.segments,
    highlights.isActive,
  );

  const className = useMemo(
    () =>
      cn([
        audioExists && "cursor-pointer hover:bg-neutral-200/60",
        !word.isFinal && ["opacity-60", "italic"],
      ]),
    [audioExists, word.isFinal],
  );

  const handleClick = useCallback(() => {
    onClickWord(word);
  }, [word, onClickWord]);

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      onContextMenu?.(word, e);
    },
    [word, onContextMenu],
  );

  return (
    <span
      onClick={handleClick}
      onContextMenu={handleContextMenu}
      className={className}
      data-word-id={word.id}
    >
      {content}
    </span>
  );
}

function useHighlightedContent(
  word: SegmentWord,
  segments: HighlightSegment[],
  isActive: boolean,
) {
  return useMemo(() => {
    const baseKey = word.id ?? word.text ?? "word";

    return segments.map((piece, index) =>
      piece.isMatch ? (
        <span
          key={`${baseKey}-match-${index}`}
          className={isActive ? "bg-yellow-500" : "bg-yellow-200/50"}
        >
          {piece.text}
        </span>
      ) : (
        <Fragment key={`${baseKey}-text-${index}`}>{piece.text}</Fragment>
      ),
    );
  }, [segments, isActive, word.id, word.text]);
}
