import { convertFileSrc } from "@tauri-apps/api/core";
import type { EditorView } from "prosemirror-view";
import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { cn } from "@hypr/utils";

import { type Attachment, Attachments } from "./attachments";
import { Enhanced } from "./enhanced";
import { Header, useAttachments, useEditorTabs } from "./header";
import { RawEditor } from "./raw";
import { SearchBar } from "./search/bar";
import { useSearch } from "./search/context";
import { Transcript } from "./transcript";

import type { NoteEditorRef } from "~/editor/session";
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
}

export const NoteInput = forwardRef<
  NoteInputHandle,
  {
    tab: Extract<Tab, { type: "sessions" }>;
    onNavigateToTitle?: (pixelWidth?: number) => void;
  }
>(({ tab, onNavigateToTitle }, ref) => {
  const editorTabs = useEditorTabs({ sessionId: tab.id });
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);
  const internalEditorRef = useRef<NoteEditorRef>(null);
  const [container, setContainer] = useState<HTMLDivElement | null>(null);
  const [view, setView] = useState<EditorView | null>(null);

  const sessionId = tab.id;

  const tabRef = useRef(tab);
  tabRef.current = tab;

  const currentTab: TabEditorView = useCurrentNoteTab(tab);

  useImperativeHandle(
    ref,
    () => ({
      focus: () => internalEditorRef.current?.commands.focus(),
      focusAtStart: () => internalEditorRef.current?.commands.focusAtStart(),
      focusAtPixelWidth: (px) =>
        internalEditorRef.current?.commands.focusAtPixelWidth(px),
      insertAtStartAndFocus: (content) =>
        internalEditorRef.current?.commands.insertAtStartAndFocus(content),
    }),
    [currentTab],
  );

  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const isMeetingInProgress =
    sessionMode === "active" ||
    sessionMode === "finalizing" ||
    sessionMode === "running_batch";

  const { scrollRef, onBeforeTabChange } = useScrollPreservation(
    currentTab.type === "enhanced"
      ? `enhanced-${currentTab.id}`
      : currentTab.type,
    {
      skipRestoration: currentTab.type === "transcript" && isMeetingInProgress,
    },
  );

  const handleTabChange = useCallback(
    (tabView: TabEditorView) => {
      onBeforeTabChange();
      updateSessionTabState(tabRef.current, {
        ...tabRef.current.state,
        view: tabView,
      });
    },
    [onBeforeTabChange, updateSessionTabState],
  );

  useTabShortcuts({
    editorTabs,
    currentTab,
    handleTabChange,
  });

  useEffect(() => {
    if (currentTab.type === "transcript" || currentTab.type === "attachments") {
      setView(null);
    } else if (currentTab.type === "raw" && isMeetingInProgress) {
      requestAnimationFrame(() => {
        internalEditorRef.current?.commands.focus();
      });
    }
  }, [currentTab, isMeetingInProgress]);

  useEffect(() => {
    const editorView = internalEditorRef.current?.view ?? null;
    if (editorView !== view) {
      setView(editorView);
    }
  });

  useCaretNearBottom({
    view,
    container,
    enabled:
      currentTab.type !== "transcript" && currentTab.type !== "attachments",
  });

  const search = useSearch();
  const showSearchBar = search?.isVisible ?? false;

  useEffect(() => {
    search?.close();
  }, [currentTab]);

  const handleContainerClick = () => {
    if (currentTab.type !== "transcript" && currentTab.type !== "attachments") {
      internalEditorRef.current?.commands.focus();
    }
  };

  return (
    <div className="-mx-2 flex h-full flex-col">
      <div className="relative px-2">
        <Header
          sessionId={sessionId}
          editorTabs={editorTabs}
          currentTab={currentTab}
          handleTabChange={handleTabChange}
        />
      </div>

      {showSearchBar && (
        <div className="px-3 pt-1">
          <SearchBar
            editorRef={internalEditorRef}
            isTranscript={currentTab.type === "transcript"}
          />
        </div>
      )}

      <div className="relative flex-1 overflow-hidden">
        <div
          ref={(node) => {
            if (
              currentTab.type !== "transcript" &&
              currentTab.type !== "attachments"
            ) {
              scrollRef.current = node;
              setContainer(node);
            } else {
              scrollRef.current = node;
              setContainer(null);
            }
          }}
          onClick={handleContainerClick}
          className={cn([
            "h-full px-3",
            currentTab.type === "transcript"
              ? "overflow-hidden"
              : ["scroll-fade-y overflow-auto", "pt-2", "pb-6"],
          ])}
        >
          {currentTab.type === "enhanced" && (
            <Enhanced
              ref={internalEditorRef}
              sessionId={sessionId}
              enhancedNoteId={currentTab.id}
              onNavigateToTitle={onNavigateToTitle}
            />
          )}
          {currentTab.type === "raw" && (
            <RawEditor
              ref={internalEditorRef}
              sessionId={sessionId}
              onNavigateToTitle={onNavigateToTitle}
            />
          )}
          {currentTab.type === "transcript" && (
            <Transcript sessionId={sessionId} scrollRef={scrollRef} />
          )}
          {currentTab.type === "attachments" && (
            <AttachmentsContent sessionId={sessionId} />
          )}
        </div>
      </div>
    </div>
  );
});

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
    "alt+t",
    () => {
      const transcriptTab = editorTabs.find((t) => t.type === "transcript");
      if (transcriptTab && currentTab.type !== "transcript") {
        handleTabChange(transcriptTab);
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

function AttachmentsContent({ sessionId }: { sessionId: string }) {
  const {
    attachments: rawAttachments,
    isLoading,
    refetch,
  } = useAttachments(sessionId);

  const attachments = useMemo<Attachment[]>(() => {
    return rawAttachments.map((info) => {
      const fileUrl = convertFileSrc(info.path);
      return {
        attachmentId: info.attachmentId,
        type: "image" as const,
        url: fileUrl,
        path: info.path,
        title: info.attachmentId,
        thumbnailUrl: fileUrl,
        addedAt: info.modifiedAt,
        isPersisted: true,
      };
    });
  }, [rawAttachments]);

  const handleRemove = useCallback(
    async (attachmentId: string) => {
      const result = await fsSyncCommands.attachmentRemove(
        sessionId,
        attachmentId,
      );
      if (result.status === "ok") {
        refetch();
      }
    },
    [sessionId, refetch],
  );

  return (
    <Attachments
      attachments={attachments}
      onRemoveAttachment={handleRemove}
      isLoading={isLoading}
    />
  );
}
