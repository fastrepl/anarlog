import { useCallback, useMemo } from "react";

import type { Operations, SegmentWord } from "@hypr/transcript";
import type { HighlightSegment } from "@hypr/transcript/ui";
import { WordSpan as SharedWordSpan } from "@hypr/transcript/ui";

import { useTranscriptSearch } from "~/session/components/note-input/transcript/search-context";
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

    return createSegments(text, query, caseSensitive, wholeWord);
  }, [isVisible, query, word.text, caseSensitive, wholeWord]);

  const isActive = word.id === activeMatchId;

  return { segments, isActive };
}

function isWordBoundaryChar(text: string, index: number): boolean {
  if (index < 0 || index >= text.length) return true;
  return !/\w/.test(text[index]);
}

function createSegments(
  rawText: string,
  query: string,
  caseSensitive: boolean,
  wholeWord: boolean,
): HighlightSegment[] {
  const text = rawText.normalize("NFC");
  const searchText = caseSensitive ? text : text.toLowerCase();

  const tokens = query
    .normalize("NFC")
    .split(/\s+/)
    .filter(Boolean)
    .map((t) => (caseSensitive ? t : t.toLowerCase()));
  if (tokens.length === 0) return [{ text, isMatch: false }];

  const ranges: { start: number; end: number }[] = [];
  for (const token of tokens) {
    let cursor = 0;
    let index = searchText.indexOf(token, cursor);
    while (index !== -1) {
      if (wholeWord) {
        const beforeOk = isWordBoundaryChar(searchText, index - 1);
        const afterOk = isWordBoundaryChar(searchText, index + token.length);
        if (beforeOk && afterOk) {
          ranges.push({ start: index, end: index + token.length });
        }
      } else {
        ranges.push({ start: index, end: index + token.length });
      }
      cursor = index + 1;
      index = searchText.indexOf(token, cursor);
    }
  }

  if (ranges.length === 0) {
    return [{ text, isMatch: false }];
  }

  ranges.sort((a, b) => a.start - b.start);
  const merged: { start: number; end: number }[] = [{ ...ranges[0] }];
  for (let i = 1; i < ranges.length; i++) {
    const last = merged[merged.length - 1];
    if (ranges[i].start <= last.end) {
      last.end = Math.max(last.end, ranges[i].end);
    } else {
      merged.push({ ...ranges[i] });
    }
  }

  const segments: HighlightSegment[] = [];
  let cursor = 0;
  for (const range of merged) {
    if (range.start > cursor) {
      segments.push({ text: text.slice(cursor, range.start), isMatch: false });
    }
    segments.push({ text: text.slice(range.start, range.end), isMatch: true });
    cursor = range.end;
  }
  if (cursor < text.length) {
    segments.push({ text: text.slice(cursor), isMatch: false });
  }

  return segments.length ? segments : [{ text, isMatch: false }];
}
