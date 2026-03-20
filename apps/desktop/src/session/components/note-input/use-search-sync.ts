import { type MutableRefObject, useEffect } from "react";

import type { TiptapEditor } from "@hypr/tiptap/editor";

import { handleEditorReplace, handleTranscriptReplace } from "./search-replace";
import {
  type SearchReplaceDetail,
  useTranscriptSearch,
} from "./transcript/search/context";

import * as main from "~/store/tinybase/store/main";
import { type EditorView } from "~/store/zustand/tabs/schema";

export function useSearchSync({
  editor,
  currentTab,
  sessionId,
  editorRef,
}: {
  editor: TiptapEditor | null;
  currentTab: EditorView;
  sessionId: string;
  editorRef: MutableRefObject<{ editor: TiptapEditor | null } | null>;
}) {
  const search = useTranscriptSearch();
  const showSearchBar = search?.isVisible ?? false;

  useEffect(() => {
    search?.close();
  }, [currentTab]);

  useEffect(() => {
    if (!editor?.storage?.searchAndReplace) return;

    const isEditorTab =
      currentTab.type !== "transcript" && currentTab.type !== "attachments";
    const query = isEditorTab && search?.isVisible ? (search.query ?? "") : "";

    editor.storage.searchAndReplace.searchTerm = query;
    editor.storage.searchAndReplace.caseSensitive =
      search?.caseSensitive ?? false;
    editor.storage.searchAndReplace.resultIndex =
      search?.currentMatchIndex ?? 0;

    try {
      editor.view.dispatch(editor.state.tr);
    } catch {
      return;
    }

    if (query) {
      requestAnimationFrame(() => {
        const el = editor.view.dom.querySelector(".search-result-current");
        el?.scrollIntoView({ behavior: "smooth", block: "center" });
      });
    }
  }, [
    editor,
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
        handleEditorReplace(detail, editorRef.current?.editor ?? null);
      }
    };
    window.addEventListener("search-replace", handler);
    return () => window.removeEventListener("search-replace", handler);
  }, [currentTab, store, indexes, checkpoints, sessionId, editorRef]);

  return { showSearchBar };
}
