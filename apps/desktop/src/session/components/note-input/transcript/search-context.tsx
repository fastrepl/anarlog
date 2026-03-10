import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { isWordBoundary } from "../search-replace";

export interface SearchOptions {
  caseSensitive: boolean;
  wholeWord: boolean;
}

interface SearchContextValue {
  query: string;
  isVisible: boolean;
  currentMatchIndex: number;
  totalMatches: number;
  activeMatchId: string | null;
  caseSensitive: boolean;
  wholeWord: boolean;
  showReplace: boolean;
  replaceQuery: string;
  onNext: () => void;
  onPrev: () => void;
  close: () => void;
  setQuery: (query: string) => void;
  toggleCaseSensitive: () => void;
  toggleWholeWord: () => void;
  toggleReplace: () => void;
  setReplaceQuery: (query: string) => void;
  replaceCurrent: () => void;
  replaceAll: () => void;
}

const SearchContext = createContext<SearchContextValue | null>(null);

export function useTranscriptSearch() {
  return useContext(SearchContext);
}

interface MatchResult {
  element: HTMLElement;
  id: string | null;
}

function prepareQuery(query: string, caseSensitive: boolean): string {
  const trimmed = query.trim().normalize("NFC");
  return caseSensitive ? trimmed : trimmed.toLowerCase();
}

function prepareText(text: string, caseSensitive: boolean): string {
  const normalized = text.normalize("NFC");
  return caseSensitive ? normalized : normalized.toLowerCase();
}

function findOccurrences(
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

function getMatchingElements(
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

function getTranscriptMatches(
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

function getEditorMatches(
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

function findSearchContainer(): HTMLElement | null {
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

export interface SearchReplaceDetail {
  query: string;
  replacement: string;
  caseSensitive: boolean;
  wholeWord: boolean;
  all: boolean;
  matchIndex: number;
}

export function SearchProvider({ children }: { children: React.ReactNode }) {
  const [isVisible, setIsVisible] = useState(false);
  const [query, setQuery] = useState("");
  const [currentMatchIndex, setCurrentMatchIndex] = useState(0);
  const [totalMatches, setTotalMatches] = useState(0);
  const [activeMatchId, setActiveMatchId] = useState<string | null>(null);
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [wholeWord, setWholeWord] = useState(false);
  const [showReplace, setShowReplace] = useState(false);
  const [replaceQuery, setReplaceQuery] = useState("");
  const matchesRef = useRef<MatchResult[]>([]);

  const opts: SearchOptions = useMemo(
    () => ({ caseSensitive, wholeWord }),
    [caseSensitive, wholeWord],
  );

  const close = useCallback(() => {
    setIsVisible(false);
    setShowReplace(false);
  }, []);

  const toggleCaseSensitive = useCallback(() => {
    setCaseSensitive((prev) => !prev);
  }, []);

  const toggleWholeWord = useCallback(() => {
    setWholeWord((prev) => !prev);
  }, []);

  const toggleReplace = useCallback(() => {
    setShowReplace((prev) => !prev);
  }, []);

  useHotkeys(
    "mod+f",
    (event) => {
      event.preventDefault();
      setIsVisible((prev) => !prev);
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [],
  );

  useHotkeys(
    "mod+h",
    (event) => {
      event.preventDefault();
      setIsVisible(true);
      setShowReplace((prev) => !prev);
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [],
  );

  useHotkeys(
    "esc",
    () => {
      close();
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [close],
  );

  useEffect(() => {
    if (!isVisible) {
      setQuery("");
      setReplaceQuery("");
      setCurrentMatchIndex(0);
      setActiveMatchId(null);
      setShowReplace(false);
      matchesRef.current = [];
    }
  }, [isVisible]);

  const runSearch = useCallback(() => {
    const container = findSearchContainer();
    if (!container || !query) {
      setTotalMatches(0);
      setCurrentMatchIndex(0);
      setActiveMatchId(null);
      matchesRef.current = [];
      return;
    }

    const matches = getMatchingElements(container, query, opts);
    matchesRef.current = matches;
    setTotalMatches(matches.length);
    setCurrentMatchIndex(0);
    setActiveMatchId(matches[0]?.id || null);

    if (matches.length > 0 && !matches[0].id) {
      matches[0].element.scrollIntoView({
        behavior: "smooth",
        block: "center",
      });
    }
  }, [query, opts]);

  useEffect(() => {
    runSearch();
  }, [runSearch]);

  const onNext = useCallback(() => {
    const matches = matchesRef.current;
    if (matches.length === 0) return;

    const nextIndex = (currentMatchIndex + 1) % matches.length;
    setCurrentMatchIndex(nextIndex);
    setActiveMatchId(matches[nextIndex]?.id || null);
    matches[nextIndex]?.element.scrollIntoView({
      behavior: "smooth",
      block: "center",
    });
  }, [currentMatchIndex]);

  const onPrev = useCallback(() => {
    const matches = matchesRef.current;
    if (matches.length === 0) return;

    const prevIndex = (currentMatchIndex - 1 + matches.length) % matches.length;
    setCurrentMatchIndex(prevIndex);
    setActiveMatchId(matches[prevIndex]?.id || null);
    matches[prevIndex]?.element.scrollIntoView({
      behavior: "smooth",
      block: "center",
    });
  }, [currentMatchIndex]);

  const replaceCurrent = useCallback(() => {
    if (!query || matchesRef.current.length === 0) return;
    const detail: SearchReplaceDetail = {
      query,
      replacement: replaceQuery,
      caseSensitive,
      wholeWord,
      all: false,
      matchIndex: currentMatchIndex,
    };
    window.dispatchEvent(new CustomEvent("search-replace", { detail }));
    setTimeout(runSearch, 50);
  }, [
    query,
    replaceQuery,
    caseSensitive,
    wholeWord,
    currentMatchIndex,
    runSearch,
  ]);

  const replaceAllFn = useCallback(() => {
    if (!query) return;
    const detail: SearchReplaceDetail = {
      query,
      replacement: replaceQuery,
      caseSensitive,
      wholeWord,
      all: true,
      matchIndex: 0,
    };
    window.dispatchEvent(new CustomEvent("search-replace", { detail }));
    setTimeout(runSearch, 50);
  }, [query, replaceQuery, caseSensitive, wholeWord, runSearch]);

  useEffect(() => {
    if (!isVisible || !activeMatchId) return;

    const container = findSearchContainer();
    if (!container) return;

    const element = container.querySelector<HTMLElement>(
      `[data-word-id="${activeMatchId}"]`,
    );

    if (element) {
      element.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  }, [isVisible, activeMatchId]);

  const value = useMemo(
    () => ({
      query,
      isVisible,
      currentMatchIndex,
      totalMatches,
      activeMatchId,
      caseSensitive,
      wholeWord,
      showReplace,
      replaceQuery,
      onNext,
      onPrev,
      close,
      setQuery,
      toggleCaseSensitive,
      toggleWholeWord,
      toggleReplace,
      setReplaceQuery,
      replaceCurrent,
      replaceAll: replaceAllFn,
    }),
    [
      query,
      isVisible,
      currentMatchIndex,
      totalMatches,
      activeMatchId,
      caseSensitive,
      wholeWord,
      showReplace,
      replaceQuery,
      onNext,
      onPrev,
      close,
      toggleCaseSensitive,
      toggleWholeWord,
      toggleReplace,
      replaceCurrent,
      replaceAllFn,
    ],
  );

  return (
    <SearchContext.Provider value={value}>{children}</SearchContext.Provider>
  );
}
