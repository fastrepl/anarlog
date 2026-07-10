import type { EditorView } from "prosemirror-view";
import { forwardRef, useCallback, useMemo, useRef } from "react";

import { parseJsonContent } from "@hypr/editor/markdown";
import {
  NoteEditor,
  type JSONContent,
  type NoteEditorRef,
} from "@hypr/editor/note";
import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { cn } from "@hypr/utils";

import { AudioDropTarget } from "./audio-drop-target";
import { useNoteFileHandlerConfig } from "./file-handler";

import { AppLinkView } from "~/editor-bridge/app-link-view";
import { useMentionConfig } from "~/editor-bridge/mention-config";
import { openEditorLink } from "~/editor-bridge/open-editor-link";
import { sessionMentionDropConfig } from "~/editor-bridge/session-mention-drop";
import { SessionNodeView } from "~/editor-bridge/session-view";
import { hasStoredNoteContent } from "~/session/components/shared";
import { useSession, useUpdateSession } from "~/session/queries";
import {
  ensureFirstLineTitle,
  extractFirstLineTitle,
} from "~/session/title-content";

const extraNodeViews = { appLink: AppLinkView, session: SessionNodeView };

export const RawEditor = forwardRef<
  NoteEditorRef,
  {
    sessionId: string;
    className?: string;
    onNavigateToTitle?: (pixelWidth?: number) => void;
    syncTasks?: boolean;
    showFormatToolbar?: boolean;
    onViewReady?: (view: EditorView) => void;
    onViewDisposed?: (view: EditorView) => void;
  }
>(
  (
    {
      sessionId,
      className,
      onNavigateToTitle,
      syncTasks = true,
      showFormatToolbar = true,
      onViewReady,
      onViewDisposed,
    },
    ref,
  ) => {
    const session = useSession(sessionId);
    const rawMd = session?.raw_md ?? "";
    const sessionTitle = session?.title;
    const updateSession = useUpdateSession(sessionId);
    const { audioDropTargetProps, fileHandlerConfig, isAudioDragActive } =
      useNoteFileHandlerConfig(sessionId);
    const initialContent = useMemo<JSONContent>(
      () => ensureFirstLineTitle(parseJsonContent(rawMd), sessionTitle),
      [rawMd, sessionTitle],
    );

    const persistChange = useCallback(
      (input: JSONContent) => {
        const title = extractFirstLineTitle(input);
        return updateSession({
          raw_md: JSON.stringify(input),
          ...(title !== null || hasStoredNoteContent(rawMd)
            ? { title: title ?? "" }
            : {}),
        });
      },
      [rawMd, updateSession],
    );

    const hasTrackedWriteRef = useRef(false);
    const trackedSessionIdRef = useRef(sessionId);
    if (trackedSessionIdRef.current !== sessionId) {
      trackedSessionIdRef.current = sessionId;
      hasTrackedWriteRef.current = false;
    }

    const hasNonEmptyText = useCallback(
      (node?: JSONContent): boolean =>
        !!node?.text?.trim() ||
        !!node?.content?.some((child: JSONContent) => hasNonEmptyText(child)),
      [],
    );

    const handleChange = useCallback(
      (input: JSONContent) => {
        void persistChange(input).catch((error) => {
          console.error("[raw-editor] failed to persist note", error);
        });

        if (!hasTrackedWriteRef.current) {
          const hasContent = hasNonEmptyText(input);
          if (hasContent) {
            hasTrackedWriteRef.current = true;
            void trackNoteEdited();
          }
        }
      },
      [persistChange, hasNonEmptyText],
    );

    const mentionConfig = useMentionConfig();

    return (
      <AudioDropTarget
        targetProps={audioDropTargetProps}
        isActive={isAudioDragActive}
      >
        <NoteEditor
          ref={ref}
          className={cn(["session-note-editor", className])}
          key={`session-${sessionId}-raw`}
          initialContent={initialContent}
          handleChange={handleChange}
          mentionConfig={mentionConfig}
          sessionMentionDropConfig={sessionMentionDropConfig}
          onNavigateToTitle={onNavigateToTitle}
          onLinkOpen={openEditorLink}
          fileHandlerConfig={fileHandlerConfig}
          taskSource={
            syncTasks ? { type: "session_raw_note", id: sessionId } : undefined
          }
          extraNodeViews={extraNodeViews}
          showFormatToolbar={showFormatToolbar}
          onViewReady={onViewReady}
          onViewDisposed={onViewDisposed}
        />
      </AudioDropTarget>
    );
  },
);

async function trackNoteEdited() {
  try {
    await analyticsCommands.event({
      event: "note_edited",
      has_content: true,
    });
  } catch (error) {
    console.error("[raw-editor] failed to record note analytics", error);
  }
}
