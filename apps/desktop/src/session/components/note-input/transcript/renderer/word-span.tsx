import { useCallback, useMemo } from "react";

import type { Operations, SegmentWord } from "@hypr/transcript";
import { WordSpan as SharedWordSpan } from "@hypr/transcript/ui";

import { useTranscriptSearch } from "~/session/components/note-input/transcript/search/context";
import { createHighlightSegments } from "~/session/components/note-input/transcript/search/matching";
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";

interface WordSpanProps {
  word: SegmentWord;
  audioExists: boolean;
  operations?: Operations;
  onClickWord: (word: SegmentWord) => void;
}

export function WordSpan(props: WordSpanProps) {
  const searchHighlights = useTranscriptSearchHighlights(props.word);

  const contextMenu = useMemo(
    () =>
      props.operations && props.word.id
        ? [
            {
              id: "delete",
              text: "Delete",
              action: () => props.operations!.onDeleteWord?.(props.word.id!),
            },
          ]
        : [],
    [props.operations, props.word.id],
  );

  const showMenu = useNativeContextMenu(contextMenu);

  const handleContextMenu = useCallback(
    (_word: SegmentWord, e: React.MouseEvent) => {
      showMenu(e);
    },
    [showMenu],
  );

  return (
    <SharedWordSpan
      word={props.word}
      audioExists={props.audioExists}
      operations={props.operations}
      searchHighlights={searchHighlights}
      onClickWord={props.onClickWord}
      onContextMenu={
        props.operations && props.word.id ? handleContextMenu : undefined
      }
    />
  );
}

function useTranscriptSearchHighlights(word: SegmentWord) {
  const search = useTranscriptSearch();
  const query = search?.query?.trim() ?? "";
  const isVisible = Boolean(search?.isVisible);
  const activeMatchId = search?.activeMatchId ?? null;
  const caseSensitive = search?.caseSensitive ?? false;
  const wholeWord = search?.wholeWord ?? false;

  const segments = useMemo(() => {
    const text = word.text ?? "";

    if (!text) {
      return [{ text: "", isMatch: false }];
    }

    if (!isVisible || !query) {
      return [{ text, isMatch: false }];
    }

    return createHighlightSegments(text, query, caseSensitive, wholeWord);
  }, [isVisible, query, word.text, caseSensitive, wholeWord]);

  const isActive = word.id === activeMatchId;

  return { segments, isActive };
}
