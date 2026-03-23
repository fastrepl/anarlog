import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useRef,
} from "react";
import { useHotkeys } from "react-hotkeys-hook";

import {
  findSearchContainer,
  getMatchingElements,
  type MatchResult,
  type SearchOptions,
} from "./matching";

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

export interface SearchReplaceDetail {
  query: string;
  replacement: string;
  caseSensitive: boolean;
  wholeWord: boolean;
  all: boolean;
  matchIndex: number;
}

interface SearchState {
  isVisible: boolean;
  query: string;
  currentMatchIndex: number;
  totalMatches: number;
  activeMatchId: string | null;
  caseSensitive: boolean;
  wholeWord: boolean;
  showReplace: boolean;
  replaceQuery: string;
}

type SearchAction =
  | { type: "toggle_visible" }
  | { type: "open_visible" }
  | { type: "close" }
  | { type: "set_query"; query: string }
  | { type: "set_replace_query"; query: string }
  | { type: "toggle_case_sensitive" }
  | { type: "toggle_whole_word" }
  | { type: "toggle_replace" }
  | {
      type: "set_matches";
      totalMatches: number;
      currentMatchIndex: number;
      activeMatchId: string | null;
    }
  | {
      type: "navigate";
      currentMatchIndex: number;
      activeMatchId: string | null;
    };

const initialState: SearchState = {
  isVisible: false,
  query: "",
  currentMatchIndex: 0,
  totalMatches: 0,
  activeMatchId: null,
  caseSensitive: false,
  wholeWord: false,
  showReplace: false,
  replaceQuery: "",
};

function searchReducer(state: SearchState, action: SearchAction): SearchState {
  switch (action.type) {
    case "toggle_visible":
      return state.isVisible
        ? { ...initialState }
        : { ...state, isVisible: true };
    case "open_visible":
      return { ...state, isVisible: true };
    case "close":
      return { ...initialState };
    case "set_query":
      return { ...state, query: action.query };
    case "set_replace_query":
      return { ...state, replaceQuery: action.query };
    case "toggle_case_sensitive":
      return { ...state, caseSensitive: !state.caseSensitive };
    case "toggle_whole_word":
      return { ...state, wholeWord: !state.wholeWord };
    case "toggle_replace":
      return { ...state, showReplace: !state.showReplace };
    case "set_matches":
      return {
        ...state,
        totalMatches: action.totalMatches,
        currentMatchIndex: action.currentMatchIndex,
        activeMatchId: action.activeMatchId,
      };
    case "navigate":
      return {
        ...state,
        currentMatchIndex: action.currentMatchIndex,
        activeMatchId: action.activeMatchId,
      };
  }
}

export function SearchProvider({ children }: { children: React.ReactNode }) {
  const [state, dispatch] = useReducer(searchReducer, initialState);
  const matchesRef = useRef<MatchResult[]>([]);

  const opts: SearchOptions = useMemo(
    () => ({ caseSensitive: state.caseSensitive, wholeWord: state.wholeWord }),
    [state.caseSensitive, state.wholeWord],
  );

  useHotkeys(
    "mod+f",
    (event) => {
      event.preventDefault();
      dispatch({ type: "toggle_visible" });
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
      dispatch({ type: "open_visible" });
      dispatch({ type: "toggle_replace" });
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
      dispatch({ type: "close" });
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [],
  );

  const runSearch = useCallback(() => {
    const container = findSearchContainer();
    if (!container || !state.query) {
      matchesRef.current = [];
      dispatch({
        type: "set_matches",
        totalMatches: 0,
        currentMatchIndex: 0,
        activeMatchId: null,
      });
      return;
    }

    const matches = getMatchingElements(container, state.query, opts);
    matchesRef.current = matches;
    dispatch({
      type: "set_matches",
      totalMatches: matches.length,
      currentMatchIndex: 0,
      activeMatchId: matches[0]?.id || null,
    });

    if (matches.length > 0 && !matches[0].id) {
      matches[0].element.scrollIntoView({
        behavior: "smooth",
        block: "center",
      });
    }
  }, [state.query, opts]);

  useEffect(() => {
    runSearch();
  }, [runSearch]);

  const onNext = useCallback(() => {
    const matches = matchesRef.current;
    if (matches.length === 0) return;

    const nextIndex = (state.currentMatchIndex + 1) % matches.length;
    dispatch({
      type: "navigate",
      currentMatchIndex: nextIndex,
      activeMatchId: matches[nextIndex]?.id || null,
    });
    matches[nextIndex]?.element.scrollIntoView({
      behavior: "smooth",
      block: "center",
    });
  }, [state.currentMatchIndex]);

  const onPrev = useCallback(() => {
    const matches = matchesRef.current;
    if (matches.length === 0) return;

    const prevIndex =
      (state.currentMatchIndex - 1 + matches.length) % matches.length;
    dispatch({
      type: "navigate",
      currentMatchIndex: prevIndex,
      activeMatchId: matches[prevIndex]?.id || null,
    });
    matches[prevIndex]?.element.scrollIntoView({
      behavior: "smooth",
      block: "center",
    });
  }, [state.currentMatchIndex]);

  const replaceCurrent = useCallback(() => {
    if (!state.query || matchesRef.current.length === 0) return;
    const detail: SearchReplaceDetail = {
      query: state.query,
      replacement: state.replaceQuery,
      caseSensitive: state.caseSensitive,
      wholeWord: state.wholeWord,
      all: false,
      matchIndex: state.currentMatchIndex,
    };
    window.dispatchEvent(new CustomEvent("search-replace", { detail }));
    setTimeout(runSearch, 50);
  }, [
    state.query,
    state.replaceQuery,
    state.caseSensitive,
    state.wholeWord,
    state.currentMatchIndex,
    runSearch,
  ]);

  const replaceAllFn = useCallback(() => {
    if (!state.query) return;
    const detail: SearchReplaceDetail = {
      query: state.query,
      replacement: state.replaceQuery,
      caseSensitive: state.caseSensitive,
      wholeWord: state.wholeWord,
      all: true,
      matchIndex: 0,
    };
    window.dispatchEvent(new CustomEvent("search-replace", { detail }));
    setTimeout(runSearch, 50);
  }, [
    state.query,
    state.replaceQuery,
    state.caseSensitive,
    state.wholeWord,
    runSearch,
  ]);

  useEffect(() => {
    if (!state.isVisible || !state.activeMatchId) return;

    const container = findSearchContainer();
    if (!container) return;

    const element = container.querySelector<HTMLElement>(
      `[data-word-id="${state.activeMatchId}"]`,
    );

    if (element) {
      element.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  }, [state.isVisible, state.activeMatchId]);

  const value = useMemo(
    () => ({
      query: state.query,
      isVisible: state.isVisible,
      currentMatchIndex: state.currentMatchIndex,
      totalMatches: state.totalMatches,
      activeMatchId: state.activeMatchId,
      caseSensitive: state.caseSensitive,
      wholeWord: state.wholeWord,
      showReplace: state.showReplace,
      replaceQuery: state.replaceQuery,
      onNext,
      onPrev,
      close: () => dispatch({ type: "close" }),
      setQuery: (query: string) => dispatch({ type: "set_query", query }),
      toggleCaseSensitive: () => dispatch({ type: "toggle_case_sensitive" }),
      toggleWholeWord: () => dispatch({ type: "toggle_whole_word" }),
      toggleReplace: () => dispatch({ type: "toggle_replace" }),
      setReplaceQuery: (query: string) =>
        dispatch({ type: "set_replace_query", query }),
      replaceCurrent,
      replaceAll: replaceAllFn,
    }),
    [state, onNext, onPrev, replaceCurrent, replaceAllFn],
  );

  return (
    <SearchContext.Provider value={value}>{children}</SearchContext.Provider>
  );
}
