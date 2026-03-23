import { type MutableRefObject, useEffect } from "react";

import { handleEditorReplace, handleTranscriptReplace } from "./search-replace";
import {
  type SearchReplaceDetail,
  useTranscriptSearch,
} from "./transcript/search/context";

import type { NoteEditorRef } from "~/editor";
import * as main from "~/store/tinybase/store/main";
import { type EditorView } from "~/store/zustand/tabs/schema";

export function useSearchSync({
  editorRef,
  currentTab,
  sessionId,
}: {
  editorRef: MutableRefObject<NoteEditorRef | null>;
  currentTab: EditorView;
  sessionId: string;
}) {
  const search = useTranscriptSearch();
  const showSearchBar = search?.isVisible ?? false;

  useEffect(() => {
    search?.close();
  }, [currentTab]);

  useEffect(() => {
    const noteRef = editorRef.current;
    if (!noteRef?.view) return;
    const { searchStorage } = noteRef;

    const isEditorTab =
      currentTab.type !== "transcript" && currentTab.type !== "attachments";
    const query = isEditorTab && search?.isVisible ? (search.query ?? "") : "";

    searchStorage.searchTerm = query;
    searchStorage.caseSensitive = search?.caseSensitive ?? false;
    searchStorage.resultIndex = search?.currentMatchIndex ?? 0;

    try {
      noteRef.view.dispatch(noteRef.view.state.tr);
    } catch {
      return;
    }

    if (query) {
      requestAnimationFrame(() => {
        const el = noteRef.view?.dom.querySelector(".search-result-current");
        el?.scrollIntoView({ behavior: "smooth", block: "center" });
      });
    }
  }, [
    editorRef,
    currentTab.type,
    search?.isVisible,
    search?.query,
    search?.caseSensitive,
    search?.currentMatchIndex,
  ]);

  const store = main.UI.useStore(main.STORE_ID);
  const indexes = main.UI.useIndexes(main.STORE_ID);
  const checkpoints = main.UI.useCheckpoints(main.STORE_ID);

  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent<SearchReplaceDetail>).detail;
      if (currentTab.type === "transcript") {
        handleTranscriptReplace(detail, store, indexes, checkpoints, sessionId);
      } else {
        handleEditorReplace(detail, editorRef.current?.view ?? null);
      }
    };
    window.addEventListener("search-replace", handler);
    return () => window.removeEventListener("search-replace", handler);
  }, [currentTab, store, indexes, checkpoints, sessionId, editorRef]);

  return { showSearchBar };
}
