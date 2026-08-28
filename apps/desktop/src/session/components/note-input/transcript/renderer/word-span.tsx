import * as stylex from "@stylexjs/stylex";
import { Fragment, memo, useMemo } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";

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

  return (
    <span
      onClick={() => canSeek && props.onClickWord(props.word)}
      {...stylex.props(
        canSeek && styles.seekable,
        !props.word.is_final && styles.interim,
      )}
      data-transcript-word-id={props.word.id}
      data-transcript-word-start-ms={props.word.start_ms}
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
          {...stylex.props(
            isActive ? styles.activeMatch : styles.inactiveMatch,
          )}
        >
          {segment.text}
        </span>
      ) : (
        <Fragment key={`${baseKey}-text-${index}`}>{segment.text}</Fragment>
      ),
    );
  }, [displayText, isActive, segments, word.id, word.text]);
}

const styles = stylex.create({
  activeMatch: {
    backgroundColor: "#eab308",
  },
  inactiveMatch: {
    backgroundColor: "rgb(254 240 138 / 0.5)",
  },
  interim: {
    fontStyle: "italic",
    opacity: 0.6,
  },
  seekable: {
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in srgb, ${colors.accent} 60%, transparent)`,
    },
    cursor: "pointer",
  },
});
