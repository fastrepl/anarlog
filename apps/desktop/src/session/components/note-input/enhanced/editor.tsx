import type { EditorView } from "prosemirror-view";
import { forwardRef, useCallback, useMemo } from "react";

import { parseJsonContent } from "@hypr/editor/markdown";
import {
  NoteEditor,
  type JSONContent,
  type NoteEditorRef,
} from "@hypr/editor/note";

import { AudioDropTarget } from "../audio-drop-target";
import { useNoteFileHandlerConfig } from "../file-handler";

import { AppLinkView } from "~/editor-bridge/app-link-view";
import { useMentionConfig } from "~/editor-bridge/mention-config";
import { openEditorLink } from "~/editor-bridge/open-editor-link";
import { sessionMentionDropConfig } from "~/editor-bridge/session-mention-drop";
import { SessionNodeView } from "~/editor-bridge/session-view";
import { hasStoredNoteContent } from "~/session/components/shared";
import { useSession, useUpdateEnhancedNoteContent } from "~/session/queries";
import {
  ensureFirstLineTitle,
  extractFirstLineTitle,
} from "~/session/title-content";

const extraNodeViews = { appLink: AppLinkView, session: SessionNodeView };

export const EnhancedEditor = forwardRef<
  NoteEditorRef,
  {
    sessionId: string;
    enhancedNoteId: string;
    content: string;
    contentOverride?: JSONContent;
    onNavigateToTitle?: (pixelWidth?: number) => void;
    onViewReady?: (view: EditorView) => void;
    onViewDisposed?: (view: EditorView) => void;
  }
>(
  (
    {
      sessionId,
      enhancedNoteId,
      content,
      contentOverride,
      onNavigateToTitle,
      onViewReady,
      onViewDisposed,
    },
    ref,
  ) => {
    const { audioDropTargetProps, fileHandlerConfig, isAudioDragActive } =
      useNoteFileHandlerConfig(sessionId);
    const sessionTitle = useSession(sessionId)?.title;
    const updateContent = useUpdateEnhancedNoteContent(
      enhancedNoteId,
      sessionId,
    );

    const initialContent = useMemo<JSONContent>(
      () =>
        ensureFirstLineTitle(
          contentOverride ?? parseJsonContent(content),
          sessionTitle,
        ),
      [content, contentOverride, sessionTitle],
    );
    const persistChanges = contentOverride === undefined;
    const editorKey = persistChanges
      ? `enhanced-note-${enhancedNoteId}`
      : `enhanced-note-${enhancedNoteId}-preview`;

    const handleChange = useCallback(
      (input: JSONContent) => {
        const title = extractFirstLineTitle(input);
        const nextTitle =
          title !== null || hasStoredNoteContent(content)
            ? (title ?? "")
            : undefined;
        void updateContent(JSON.stringify(input), nextTitle).catch((error) => {
          console.error("[enhanced-editor] failed to persist summary", error);
        });
      },
      [content, updateContent],
    );

    const mentionConfig = useMentionConfig();

    return (
      <AudioDropTarget
        className="h-full"
        targetProps={audioDropTargetProps}
        isActive={isAudioDragActive}
      >
        <NoteEditor
          ref={ref}
          className="session-note-editor enhanced-summary-editor"
          key={editorKey}
          initialContent={initialContent}
          handleChange={persistChanges ? handleChange : undefined}
          mentionConfig={mentionConfig}
          sessionMentionDropConfig={sessionMentionDropConfig}
          onNavigateToTitle={onNavigateToTitle}
          onLinkOpen={openEditorLink}
          fileHandlerConfig={fileHandlerConfig}
          taskSource={
            persistChanges
              ? { type: "enhanced_note", id: enhancedNoteId }
              : undefined
          }
          extraNodeViews={extraNodeViews}
          onViewReady={onViewReady}
          onViewDisposed={onViewDisposed}
          syncContentWhenFocused={!persistChanges}
        />
      </AudioDropTarget>
    );
  },
);
