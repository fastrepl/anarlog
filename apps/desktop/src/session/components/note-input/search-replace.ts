import type { TiptapEditor } from "@hypr/tiptap/editor";

import type { SearchReplaceDetail } from "./transcript/search/context";

import * as main from "~/store/tinybase/store/main";
import { parseTranscriptWords, updateTranscriptWords } from "~/stt/utils";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;
type Indexes = ReturnType<typeof main.UI.useIndexes>;
type Checkpoints = ReturnType<typeof main.UI.useCheckpoints>;

export function isWordBoundary(text: string, index: number): boolean {
  if (index < 0 || index >= text.length) return true;
  return !/\w/.test(text[index]);
}

function replaceInText(
  text: string,
  query: string,
  replacement: string,
  caseSensitive: boolean,
  wholeWord: boolean,
  all: boolean,
  nth: number,
): string {
  let searchText = caseSensitive ? text : text.toLowerCase();
  const searchQuery = caseSensitive ? query : query.toLowerCase();
  let count = 0;
  let from = 0;

  while (from <= searchText.length - searchQuery.length) {
    const idx = searchText.indexOf(searchQuery, from);
    if (idx === -1) break;

    if (wholeWord) {
      const beforeOk = isWordBoundary(searchText, idx - 1);
      const afterOk = isWordBoundary(searchText, idx + searchQuery.length);
      if (!beforeOk || !afterOk) {
        from = idx + 1;
        continue;
      }
    }

    if (all || count === nth) {
      const before = text.slice(0, idx);
      const after = text.slice(idx + query.length);
      if (all) {
        text = before + replacement + after;
        searchText = caseSensitive ? text : text.toLowerCase();
        from = idx + replacement.length;
        continue;
      }
      return before + replacement + after;
    }

    count++;
    from = idx + 1;
  }

  return text;
}

export function handleTranscriptReplace(
  detail: SearchReplaceDetail,
  store: Store | undefined,
  indexes: Indexes,
  checkpoints: Checkpoints,
  sessionId: string,
) {
  if (!store || !indexes || !checkpoints) return;

  const transcriptIds = indexes.getSliceRowIds(
    main.INDEXES.transcriptBySession,
    sessionId,
  );
  if (!transcriptIds) return;

  const searchQuery = detail.caseSensitive
    ? detail.query
    : detail.query.toLowerCase();

  let globalMatchIndex = 0;

  for (const transcriptId of transcriptIds) {
    const words = parseTranscriptWords(store, transcriptId);
    if (words.length === 0) continue;

    type WordPosition = { start: number; end: number; wordIndex: number };
    const wordPositions: WordPosition[] = [];
    let fullText = "";

    for (let i = 0; i < words.length; i++) {
      const text = (words[i].text ?? "").normalize("NFC");
      if (i > 0) fullText += " ";
      const start = fullText.length;
      fullText += text;
      wordPositions.push({ start, end: fullText.length, wordIndex: i });
    }

    const searchText = detail.caseSensitive ? fullText : fullText.toLowerCase();
    let from = 0;

    type Match = { textPos: number; wordIndex: number; offsetInWord: number };
    const matches: Match[] = [];

    while (from <= searchText.length - searchQuery.length) {
      const idx = searchText.indexOf(searchQuery, from);
      if (idx === -1) break;

      if (detail.wholeWord) {
        const beforeOk = isWordBoundary(searchText, idx - 1);
        const afterOk = isWordBoundary(searchText, idx + searchQuery.length);
        if (!beforeOk || !afterOk) {
          from = idx + 1;
          continue;
        }
      }

      for (let i = 0; i < wordPositions.length; i++) {
        const { start, end, wordIndex } = wordPositions[i];
        if (idx >= start && idx < end) {
          matches.push({
            textPos: idx,
            wordIndex,
            offsetInWord: idx - start,
          });
          break;
        }
        if (
          i < wordPositions.length - 1 &&
          idx >= end &&
          idx < wordPositions[i + 1].start
        ) {
          matches.push({
            textPos: idx,
            wordIndex: wordPositions[i + 1].wordIndex,
            offsetInWord: 0,
          });
          break;
        }
      }

      from = idx + 1;
    }

    let changed = false;

    if (detail.all) {
      for (const match of matches) {
        const word = words[match.wordIndex];
        const originalText = word.text ?? "";
        word.text = replaceInText(
          originalText,
          detail.query,
          detail.replacement,
          detail.caseSensitive,
          detail.wholeWord,
          true,
          0,
        );
        if (word.text !== originalText) changed = true;
      }
    } else {
      for (const match of matches) {
        if (globalMatchIndex === detail.matchIndex) {
          const word = words[match.wordIndex];
          const originalText = word.text ?? "";
          const searchTextInWord = detail.caseSensitive
            ? originalText
            : originalText.toLowerCase();
          const searchQueryInWord = detail.caseSensitive
            ? detail.query
            : detail.query.toLowerCase();

          let nthInWord = 0;
          let pos = 0;
          while (pos <= searchTextInWord.length - searchQueryInWord.length) {
            const foundIdx = searchTextInWord.indexOf(searchQueryInWord, pos);
            if (foundIdx === -1) break;

            if (detail.wholeWord) {
              const beforeOk = isWordBoundary(searchTextInWord, foundIdx - 1);
              const afterOk = isWordBoundary(
                searchTextInWord,
                foundIdx + searchQueryInWord.length,
              );
              if (!beforeOk || !afterOk) {
                pos = foundIdx + 1;
                continue;
              }
            }

            if (foundIdx === match.offsetInWord) {
              break;
            }
            nthInWord++;
            pos = foundIdx + 1;
          }

          word.text = replaceInText(
            originalText,
            detail.query,
            detail.replacement,
            detail.caseSensitive,
            detail.wholeWord,
            false,
            nthInWord,
          );
          changed = true;
          break;
        }
        globalMatchIndex++;
      }
    }

    if (changed) {
      updateTranscriptWords(store, transcriptId, words);
      checkpoints.addCheckpoint("replace_word");
      if (!detail.all) return;
    }
  }
}

export function handleEditorReplace(
  detail: SearchReplaceDetail,
  editor: TiptapEditor | null,
) {
  if (!editor) return;

  const doc = editor.state.doc;
  const searchQuery = detail.caseSensitive
    ? detail.query
    : detail.query.toLowerCase();

  type TextNodeWithPosition = { text: string; pos: number };
  const textNodesWithPosition: TextNodeWithPosition[] = [];
  let index = 0;

  doc.descendants((node, pos) => {
    if (node.isText) {
      if (textNodesWithPosition[index]) {
        textNodesWithPosition[index] = {
          text: textNodesWithPosition[index].text + node.text,
          pos: textNodesWithPosition[index].pos,
        };
      } else {
        textNodesWithPosition[index] = {
          text: node.text ?? "",
          pos,
        };
      }
    } else {
      index += 1;
    }
  });

  type Hit = { from: number; to: number };
  const hits: Hit[] = [];

  for (const entry of textNodesWithPosition) {
    if (!entry) continue;
    const { text, pos } = entry;

    const searchText = detail.caseSensitive ? text : text.toLowerCase();
    let from = 0;

    while (from <= searchText.length - searchQuery.length) {
      const idx = searchText.indexOf(searchQuery, from);
      if (idx === -1) break;

      if (detail.wholeWord) {
        const beforeOk = isWordBoundary(searchText, idx - 1);
        const afterOk = isWordBoundary(searchText, idx + searchQuery.length);
        if (!beforeOk || !afterOk) {
          from = idx + 1;
          continue;
        }
      }

      hits.push({
        from: pos + idx,
        to: pos + idx + detail.query.length,
      });
      from = idx + 1;
    }
  }

  if (hits.length === 0) return;

  const toReplace = detail.all ? hits : [hits[detail.matchIndex]];
  if (!toReplace[0]) return;

  let offset = 0;
  const tr = editor.state.tr;

  for (const hit of toReplace) {
    const adjustedFrom = hit.from + offset;
    const adjustedTo = hit.to + offset;
    if (detail.replacement) {
      tr.replaceWith(
        adjustedFrom,
        adjustedTo,
        editor.state.schema.text(detail.replacement),
      );
    } else {
      tr.delete(adjustedFrom, adjustedTo);
    }
    offset += detail.replacement.length - detail.query.length;
  }

  editor.view.dispatch(tr);
}
