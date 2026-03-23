import { convertFileSrc } from "@tauri-apps/api/core";
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
import type { TiptapEditor } from "@hypr/tiptap/editor";
import {
  ScrollFadeOverlay,
  useScrollFade,
} from "@hypr/ui/components/ui/scroll-fade";
import { cn } from "@hypr/utils";

import { type Attachment, Attachments } from "./attachments";
import { Enhanced } from "./enhanced";
import { Header, useAttachments, useEditorTabs } from "./header";
import { RawEditor } from "./raw";
import { Transcript } from "./transcript";
import { SearchBar } from "./transcript/search/bar";
import { useSearchSync } from "./use-search-sync";

import { useCaretNearBottom } from "~/session/components/caret-position-context";
import { useCurrentNoteTab } from "~/session/components/shared";
import { useScrollPreservation } from "~/shared/hooks/useScrollPreservation";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { type EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";

export const NoteInput = forwardRef<
  { editor: TiptapEditor | null },
  {
    tab: Extract<Tab, { type: "sessions" }>;
    onNavigateToTitle?: () => void;
  }
>(({ tab, onNavigateToTitle }, ref) => {
  const editorTabs = useEditorTabs({ sessionId: tab.id });
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);
  const internalEditorRef = useRef<{ editor: TiptapEditor | null }>(null);
  const [container, setContainer] = useState<HTMLDivElement | null>(null);
  const [editor, setEditor] = useState<TiptapEditor | null>(null);
  const [isEditing, setIsEditing] = useState(false);

  const sessionId = tab.id;

  const tabRef = useRef(tab);
  tabRef.current = tab;

  const currentTab: EditorView = useCurrentNoteTab(tab);
  useImperativeHandle(
    ref,
    () => internalEditorRef.current ?? { editor: null },
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

  const fadeRef = useRef<HTMLDivElement>(null);
  const { atStart, atEnd } = useScrollFade(fadeRef, "vertical", [currentTab]);

  const handleTabChange = useCallback(
    (view: EditorView) => {
      onBeforeTabChange();
      updateSessionTabState(tabRef.current, {
        ...tabRef.current.state,
        view,
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
      internalEditorRef.current = { editor: null };
      setEditor(null);
    } else if (currentTab.type === "raw" && isMeetingInProgress) {
      requestAnimationFrame(() => {
        internalEditorRef.current?.editor?.commands.focus();
      });
    }
  }, [currentTab, isMeetingInProgress]);

  useEffect(() => {
    const editorInstance = internalEditorRef.current?.editor ?? null;
    if (editorInstance !== editor) {
      setEditor(editorInstance);
    }
  });

  useEffect(() => {
    const handleContentTransfer = (e: Event) => {
      const customEvent = e as CustomEvent<{ content: string }>;
      const content = customEvent.detail.content;
      const editorInstance = internalEditorRef.current?.editor;

      if (editorInstance && content) {
        editorInstance.commands.insertContentAt(0, content);
        editorInstance.commands.setTextSelection(0);
        editorInstance.commands.focus();
      }
    };

    const handleMoveToEditorStart = () => {
      const editorInstance = internalEditorRef.current?.editor;
      if (editorInstance) {
        editorInstance.commands.setTextSelection(0);
        editorInstance.commands.focus();
      }
    };

    const handleMoveToEditorPosition = (e: Event) => {
      const customEvent = e as CustomEvent<{ pixelWidth: number }>;
      const pixelWidth = customEvent.detail.pixelWidth;
      const editorInstance = internalEditorRef.current?.editor;

      if (editorInstance) {
        const editorDom = editorInstance.view.dom;
        const firstTextNode = editorDom.querySelector(".ProseMirror > *");

        if (firstTextNode) {
          const editorStyle = window.getComputedStyle(firstTextNode);
          const canvas = document.createElement("canvas");
          const ctx = canvas.getContext("2d");

          if (ctx) {
            ctx.font = `${editorStyle.fontWeight} ${editorStyle.fontSize} ${editorStyle.fontFamily}`;

            const firstBlock = editorInstance.state.doc.firstChild;
            if (firstBlock && firstBlock.textContent) {
              const text = firstBlock.textContent;
              let charPos = 0;

              for (let i = 0; i <= text.length; i++) {
                const currentWidth = ctx.measureText(text.slice(0, i)).width;
                if (currentWidth >= pixelWidth) {
                  charPos = i;
                  break;
                }
                charPos = i;
              }

              const targetPos = Math.min(
                charPos,
                editorInstance.state.doc.content.size - 1,
              );
              editorInstance.commands.setTextSelection(targetPos);
              editorInstance.commands.focus();
              return;
            }
          }
        }

        editorInstance.commands.setTextSelection(0);
        editorInstance.commands.focus();
      }
    };

    window.addEventListener("title-content-transfer", handleContentTransfer);
    window.addEventListener(
      "title-move-to-editor-start",
      handleMoveToEditorStart,
    );
    window.addEventListener(
      "title-move-to-editor-position",
      handleMoveToEditorPosition,
    );
    return () => {
      window.removeEventListener(
        "title-content-transfer",
        handleContentTransfer,
      );
      window.removeEventListener(
        "title-move-to-editor-start",
        handleMoveToEditorStart,
      );
      window.removeEventListener(
        "title-move-to-editor-position",
        handleMoveToEditorPosition,
      );
    };
  }, []);

  useCaretNearBottom({
    editor,
    container,
    enabled:
      currentTab.type !== "transcript" && currentTab.type !== "attachments",
  });

  const { showSearchBar } = useSearchSync({
    editor,
    currentTab,
    sessionId,
    editorRef: internalEditorRef,
  });

  const handleContainerClick = () => {
    if (currentTab.type !== "transcript" && currentTab.type !== "attachments") {
      internalEditorRef.current?.editor?.commands.focus();
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
          isEditing={isEditing}
          setIsEditing={setIsEditing}
        />
      </div>

      {showSearchBar && (
        <div className="px-3 pt-1">
          <SearchBar />
        </div>
      )}

      <div className="relative flex-1 overflow-hidden">
        <div
          ref={(node) => {
            fadeRef.current = node;
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
              : ["overflow-auto", "pt-2", "pb-6"],
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
            <Transcript
              sessionId={sessionId}
              isEditing={isEditing}
              scrollRef={scrollRef}
            />
          )}
          {currentTab.type === "attachments" && (
            <AttachmentsContent sessionId={sessionId} />
          )}
        </div>
        {!atStart && <ScrollFadeOverlay position="top" />}
        {!atEnd && <ScrollFadeOverlay position="bottom" />}
      </div>
    </div>
  );
});

function useTabShortcuts({
  editorTabs,
  currentTab,
  handleTabChange,
}: {
  editorTabs: EditorView[];
  currentTab: EditorView;
  handleTabChange: (view: EditorView) => void;
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
