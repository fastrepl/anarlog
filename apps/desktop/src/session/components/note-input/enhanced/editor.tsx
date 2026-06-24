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
import {
  ensureFirstLineTitle,
  extractFirstLineTitle,
} from "~/session/title-content";
import * as main from "~/store/tinybase/store/main";

const extraNodeViews = { appLink: AppLinkView, session: SessionNodeView };

export const EnhancedEditor = forwardRef<
  NoteEditorRef,
  {
    sessionId: string;
    enhancedNoteId: string;
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
      contentOverride,
      onNavigateToTitle,
      onViewReady,
      onViewDisposed,
    },
    ref,
  ) => {
    const { audioDropTargetProps, fileHandlerConfig, isAudioDragActive } =
      useNoteFileHandlerConfig(sessionId);
    const content = main.UI.useCell(
      "enhanced_notes",
      enhancedNoteId,
      "content",
      main.STORE_ID,
    );
    const sessionTitle = main.UI.useCell(
      "sessions",
      sessionId,
      "title",
      main.STORE_ID,
    ) as string | undefined;

    const initialContent = useMemo<JSONContent>(
      () =>
        ensureFirstLineTitle(
          contentOverride ?? parseJsonContent(content as string),
          sessionTitle,
        ),
      [content, contentOverride, sessionTitle],
    );
    const persistChanges = contentOverride === undefined;
    const editorKey = persistChanges
      ? `enhanced-note-${enhancedNoteId}`
      : `enhanced-note-${enhancedNoteId}-preview`;

    const persistContent = main.UI.useSetPartialRowCallback(
      "enhanced_notes",
      enhancedNoteId,
      (input: JSONContent) => ({ content: JSON.stringify(input) }),
      [],
      main.STORE_ID,
    );
    const persistSessionTitle = main.UI.useSetPartialRowCallback(
      "sessions",
      sessionId,
      (title: string) => ({ title }),
      [],
      main.STORE_ID,
    );
    const handleChange = useCallback(
      (input: JSONContent) => {
        persistContent(input);

        const title = extractFirstLineTitle(input);
        if (title !== null || hasStoredNoteContent(content)) {
          persistSessionTitle(title ?? "");
        }
      },
      [content, persistContent, persistSessionTitle],
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
