import { isWordBoundary } from "../../search-replace";

export interface SearchOptions {
  caseSensitive: boolean;
  wholeWord: boolean;
}

export interface MatchResult {
  element: HTMLElement;
  id: string | null;
}

export function prepareQuery(query: string, caseSensitive: boolean): string {
  const trimmed = query.trim().normalize("NFC");
  return caseSensitive ? trimmed : trimmed.toLowerCase();
}

export function prepareText(text: string, caseSensitive: boolean): string {
  const normalized = text.normalize("NFC");
  return caseSensitive ? normalized : normalized.toLowerCase();
}

export function findOccurrences(
  text: string,
  query: string,
  wholeWord: boolean,
): number[] {
  const indices: number[] = [];
  let from = 0;
  while (from <= text.length - query.length) {
    const idx = text.indexOf(query, from);
    if (idx === -1) break;
    if (wholeWord) {
      const beforeOk = isWordBoundary(text, idx - 1);
      const afterOk = isWordBoundary(text, idx + query.length);
      if (beforeOk && afterOk) {
        indices.push(idx);
      }
    } else {
      indices.push(idx);
    }
    from = idx + 1;
  }
  return indices;
}

export function getMatchingElements(
  container: HTMLElement | null,
  query: string,
  opts: SearchOptions,
): MatchResult[] {
  if (!container || !query) return [];

  const prepared = prepareQuery(query, opts.caseSensitive);
  if (!prepared) return [];

  const wordSpans = Array.from(
    container.querySelectorAll<HTMLElement>("[data-word-id]"),
  );

  if (wordSpans.length > 0) {
    return getTranscriptMatches(wordSpans, prepared, opts);
  }

  const proseMirror =
    container.querySelector<HTMLElement>(".ProseMirror") ??
    (container.classList.contains("ProseMirror") ? container : null);
  if (proseMirror) {
    return getEditorMatches(proseMirror, prepared, opts);
  }

  return [];
}

export function getTranscriptMatches(
  allSpans: HTMLElement[],
  prepared: string,
  opts: SearchOptions,
): MatchResult[] {
  const spanPositions: { start: number; end: number }[] = [];
  let fullText = "";

  for (let i = 0; i < allSpans.length; i++) {
    const text = (allSpans[i].textContent || "").normalize("NFC");
    if (i > 0) fullText += " ";
    const start = fullText.length;
    fullText += text;
    spanPositions.push({ start, end: fullText.length });
  }

  const searchText = prepareText(fullText, opts.caseSensitive);
  const indices = findOccurrences(searchText, prepared, opts.wholeWord);
  const result: MatchResult[] = [];

  for (const idx of indices) {
    for (let i = 0; i < spanPositions.length; i++) {
      const { start, end } = spanPositions[i];
      if (idx >= start && idx < end) {
        result.push({
          element: allSpans[i],
          id: allSpans[i].dataset.wordId || null,
        });
        break;
      }
      if (
        i < spanPositions.length - 1 &&
        idx >= end &&
        idx < spanPositions[i + 1].start
      ) {
        result.push({
          element: allSpans[i + 1],
          id: allSpans[i + 1].dataset.wordId || null,
        });
        break;
      }
    }
  }

  return result;
}

export function getEditorMatches(
  proseMirror: HTMLElement,
  prepared: string,
  opts: SearchOptions,
): MatchResult[] {
  const blocks = Array.from(
    proseMirror.querySelectorAll<HTMLElement>(
      "p, h1, h2, h3, h4, h5, h6, li, blockquote, td, th",
    ),
  );

  const result: MatchResult[] = [];

  for (const block of blocks) {
    const text = prepareText(block.textContent || "", opts.caseSensitive);
    const indices = findOccurrences(text, prepared, opts.wholeWord);
    for (const _ of indices) {
      result.push({ element: block, id: null });
    }
  }

  return result;
}

export function findSearchContainer(): HTMLElement | null {
  if (typeof document === "undefined") return null;

  const transcript = document.querySelector<HTMLElement>(
    "[data-transcript-container]",
  );
  if (transcript) return transcript;

  const proseMirror = document.querySelector<HTMLElement>(".ProseMirror");
  if (proseMirror) {
    return proseMirror.parentElement ?? proseMirror;
  }

  return null;
}

function isWordBoundaryChar(text: string, index: number): boolean {
  if (index < 0 || index >= text.length) return true;
  return !/\w/.test(text[index]);
}

export function createHighlightSegments(
  rawText: string,
  query: string,
  caseSensitive: boolean,
  wholeWord: boolean,
): { text: string; isMatch: boolean }[] {
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

  const segments: { text: string; isMatch: boolean }[] = [];
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
