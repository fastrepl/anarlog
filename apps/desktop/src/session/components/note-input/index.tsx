import type { EditorView } from "prosemirror-view";
import {
  forwardRef,
  type UIEventHandler,
  useCallback,
  useDeferredValue,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
} from "react";
import { useHotkeys } from "react-hotkeys-hook";

import type { NoteEditorRef } from "@hypr/editor/note";
import { cn } from "@hypr/utils";

import { Enhanced } from "./enhanced";
import { Header, useEditorTabs } from "./header";
import { RawEditor } from "./raw";
import { SearchBar } from "./search/bar";
import { useSearch } from "./search/context";
import { Transcript } from "./transcript";

import { useCaretNearBottom } from "~/session/components/caret-position-context";
import { useCurrentNoteTab } from "~/session/components/shared";
import { useScrollPreservation } from "~/shared/hooks/useScrollPreservation";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { type EditorView as TabEditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";

export interface NoteInputHandle {
  focus: () => void;
  focusAtStart: () => void;
  focusAtPixelWidth: (pixelWidth: number) => void;
  insertAtStartAndFocus: (content: string) => void;
  prepareForTabChange: () => void;
}

export const NoteInput = forwardRef<
  NoteInputHandle,
  {
    tab: Extract<Tab, { type: "sessions" }>;
    onNavigateToTitle?: (pixelWidth?: number) => void;
    onScroll?: UIEventHandler<HTMLDivElement>;
    editorTabs?: TabEditorView[];
    currentTab?: TabEditorView;
    handleTabChange?: (view: TabEditorView) => void;
    hideHeader?: boolean;
  }
>(
  (
    {
      tab,
      onNavigateToTitle,
      onScroll,
      editorTabs: providedEditorTabs,
      currentTab: providedCurrentTab,
      handleTabChange: providedHandleTabChange,
      hideHeader = false,
    },
    ref,
  ) => {
    const fallbackEditorTabs = useEditorTabs({ sessionId: tab.id });
    const updateSessionTabState = useTabs(
      (state) => state.updateSessionTabState,
    );
    const internalEditorRef = useRef<NoteEditorRef>(null);
    const [container, setContainer] = useState<HTMLDivElement | null>(null);
    const [view, setView] = useState<EditorView | null>(null);

    const sessionId = tab.id;

    const tabRef = useRef(tab);
    tabRef.current = tab;

    const fallbackCurrentTab: TabEditorView = useCurrentNoteTab(tab);
    const editorTabs = providedEditorTabs ?? fallbackEditorTabs;
    const currentTab = providedCurrentTab ?? fallbackCurrentTab;
    const deferredCurrentTab = useDeferredValue(currentTab);
    const renderedCurrentTab = editorTabs.some((editorTab) =>
      isSameEditorView(editorTab, deferredCurrentTab),
    )
      ? deferredCurrentTab
      : currentTab;

    const sessionMode = useListener((state) => state.getSessionMode(sessionId));
    const isMeetingInProgress =
      sessionMode === "active" ||
      sessionMode === "finalizing" ||
      sessionMode === "running_batch";

    const { scrollRef, onBeforeTabChange } = useScrollPreservation(
      renderedCurrentTab.type === "enhanced"
        ? `enhanced-${renderedCurrentTab.id}`
        : renderedCurrentTab.type,
    );

    useImperativeHandle(
      ref,
      () => ({
        focus: () => internalEditorRef.current?.commands.focus(),
        focusAtStart: () => internalEditorRef.current?.commands.focusAtStart(),
        focusAtPixelWidth: (px) =>
          internalEditorRef.current?.commands.focusAtPixelWidth(px),
        insertAtStartAndFocus: (content) =>
          internalEditorRef.current?.commands.insertAtStartAndFocus(content),
        prepareForTabChange: onBeforeTabChange,
      }),
      [currentTab, onBeforeTabChange],
    );

    const handleTabChange = useCallback(
      (tabView: TabEditorView) => {
        if (isSameEditorView(tabView, currentTab)) {
          return;
        }

        onBeforeTabChange();
        if (providedHandleTabChange) {
          providedHandleTabChange(tabView);
        } else {
          updateSessionTabState(tabRef.current, {
            ...tabRef.current.state,
            view: tabView,
          });
        }
      },
      [
        currentTab,
        onBeforeTabChange,
        providedHandleTabChange,
        updateSessionTabState,
      ],
    );

    useTabShortcuts({
      editorTabs,
      currentTab,
      handleTabChange,
    });

    useEffect(() => {
      if (renderedCurrentTab.type === "raw" && isMeetingInProgress) {
        requestAnimationFrame(() => {
          internalEditorRef.current?.commands.focus();
        });
      }
    }, [renderedCurrentTab, isMeetingInProgress]);

    const handleViewReady = useCallback((editorView: EditorView) => {
      setView(editorView);
    }, []);

    const handleViewDisposed = useCallback((editorView: EditorView) => {
      setView((currentView) =>
        currentView === editorView ? null : currentView,
      );
    }, []);

    useCaretNearBottom({
      view,
      container,
      enabled: true,
    });

    const search = useSearch();
    const showSearchBar = search?.isVisible ?? false;
    const isEditableTab =
      renderedCurrentTab.type === "enhanced" ||
      renderedCurrentTab.type === "raw";

    useEffect(() => {
      search?.close();
    }, [currentTab]);

    const handleContainerClick = () => {
      if (!isEditableTab) {
        return;
      }

      internalEditorRef.current?.commands.focus();
    };

    return (
      <div className="-mx-2 flex h-full flex-col">
        {!hideHeader && (
          <div className="relative px-2">
            <Header
              sessionId={sessionId}
              editorTabs={editorTabs}
              currentTab={currentTab}
              handleTabChange={handleTabChange}
              isTranscribing={isMeetingInProgress}
            />
          </div>
        )}

        {showSearchBar && isEditableTab && (
          <div className="px-3 pt-1">
            <SearchBar editorRef={internalEditorRef} />
          </div>
        )}

        <div className="relative flex-1 overflow-hidden">
          <div
            ref={(node) => {
              scrollRef.current = node;
              setContainer(node);
            }}
            onClick={handleContainerClick}
            onScroll={onScroll}
            className={cn([
              "h-full px-3",
              "pt-2",
              renderedCurrentTab.type === "transcript"
                ? "overflow-hidden pb-0"
                : "scroll-fade-y overflow-auto pb-6",
            ])}
          >
            {renderedCurrentTab.type === "enhanced" && (
              <Enhanced
                ref={internalEditorRef}
                sessionId={sessionId}
                enhancedNoteId={renderedCurrentTab.id}
                onNavigateToTitle={onNavigateToTitle}
                onViewReady={handleViewReady}
                onViewDisposed={handleViewDisposed}
              />
            )}
            {renderedCurrentTab.type === "raw" && (
              <RawEditor
                ref={internalEditorRef}
                sessionId={sessionId}
                onNavigateToTitle={onNavigateToTitle}
                onViewReady={handleViewReady}
                onViewDisposed={handleViewDisposed}
              />
            )}
            {renderedCurrentTab.type === "transcript" && (
              <Transcript sessionId={sessionId} scrollRef={scrollRef} />
            )}
          </div>
        </div>
      </div>
    );
  },
);

function isSameEditorView(left: TabEditorView, right: TabEditorView): boolean {
  if (left.type !== right.type) {
    return false;
  }

  if (left.type === "enhanced" && right.type === "enhanced") {
    return left.id === right.id;
  }

  return true;
}

function useTabShortcuts({
  editorTabs,
  currentTab,
  handleTabChange,
}: {
  editorTabs: TabEditorView[];
  currentTab: TabEditorView;
  handleTabChange: (view: TabEditorView) => void;
}) {
  useHotkeys(
    "alt+s",
    () => {
      const enhancedTabs = editorTabs.filter((t) => t.type === "enhanced");
      if (enhancedTabs.length === 0) return;

      if (currentTab.type === "enhanced") {
        const currentIndex = enhancedTabs.findIndex(
          (t) => t.type === "enhanced" && t.id === currentTab.id,
        );
        const nextIndex = (currentIndex + 1) % enhancedTabs.length;
        handleTabChange(enhancedTabs[nextIndex]);
      } else {
        handleTabChange(enhancedTabs[0]);
      }
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [currentTab, editorTabs, handleTabChange],
  );

  useHotkeys(
    "alt+m",
    () => {
      const rawTab = editorTabs.find((t) => t.type === "raw");
      if (rawTab && currentTab.type !== "raw") {
        handleTabChange(rawTab);
      }
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [currentTab, editorTabs, handleTabChange],
  );

  useHotkeys(
    "ctrl+alt+left",
    () => {
      const currentIndex = editorTabs.findIndex(
        (t) =>
          (t.type === "enhanced" &&
            currentTab.type === "enhanced" &&
            t.id === currentTab.id) ||
          (t.type === currentTab.type && t.type !== "enhanced"),
      );
      if (currentIndex > 0) {
        handleTabChange(editorTabs[currentIndex - 1]);
      }
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [currentTab, editorTabs, handleTabChange],
  );

  useHotkeys(
    "ctrl+alt+right",
    () => {
      const currentIndex = editorTabs.findIndex(
        (t) =>
          (t.type === "enhanced" &&
            currentTab.type === "enhanced" &&
            t.id === currentTab.id) ||
          (t.type === currentTab.type && t.type !== "enhanced"),
      );
      if (currentIndex >= 0 && currentIndex < editorTabs.length - 1) {
        handleTabChange(editorTabs[currentIndex + 1]);
      }
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [currentTab, editorTabs, handleTabChange],
  );
}
