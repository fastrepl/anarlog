import type { EditorView } from "prosemirror-view";
import { forwardRef, memo, useCallback, useMemo } from "react";

import { parseJsonContent } from "@hypr/editor/markdown";
import {
  NoteEditor,
  type JSONContent,
  type NoteEditorRef,
  normalizePortableAttachmentUrls,
} from "@hypr/editor/note";

import { AudioDropTarget } from "../audio-drop-target";
import { useNoteFileHandlerConfig } from "../file-handler";

import { AppLinkView } from "~/editor-bridge/app-link-view";
import { useMentionConfig } from "~/editor-bridge/mention-config";
import { openEditorLink } from "~/editor-bridge/open-editor-link";
import { sessionMentionDropConfig } from "~/editor-bridge/session-mention-drop";
import { SessionNodeView } from "~/editor-bridge/session-view";
import { useSessionCommentAnchors } from "~/session-sharing/comment-anchors";
import { hasStoredNoteContent } from "~/session/components/shared";
import { useAttachmentResolver } from "~/session/hooks/useAttachmentResolver";
import { useUpdateEnhancedNoteContent } from "~/session/queries";
import {
  ensureFirstLineTitle,
  extractFirstLineTitle,
  documentTitlePlaceholder,
} from "~/session/title-content";

const extraNodeViews = { appLink: AppLinkView, session: SessionNodeView };

const EnhancedEditorInner = forwardRef<
  NoteEditorRef,
  {
    sessionId: string;
    sessionTitle: string;
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
      sessionTitle,
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
    const resolveAttachment = useAttachmentResolver(sessionId);
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
        const portableInput = normalizePortableAttachmentUrls(input);
        const title = extractFirstLineTitle(portableInput);
        const nextTitle =
          title !== null || hasStoredNoteContent(content)
            ? (title ?? "")
            : undefined;
        void updateContent(JSON.stringify(portableInput), nextTitle).catch(
          (error) => {
            console.error("[enhanced-editor] failed to persist summary", error);
          },
        );
      },
      [content, updateContent],
    );

    const mentionConfig = useMentionConfig();
    const commentAnchors = useSessionCommentAnchors(sessionId);

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
          resolveAttachment={resolveAttachment}
          handleChange={persistChanges ? handleChange : undefined}
          placeholderComponent={documentTitlePlaceholder}
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
          commentAnchorsEnabled
          onViewReady={(view) => {
            commentAnchors.onViewReady(view);
            onViewReady?.(view);
          }}
          onViewDisposed={(view) => {
            commentAnchors.onViewDisposed(view);
            onViewDisposed?.(view);
          }}
          syncContentWhenFocused={!persistChanges}
        />
      </AudioDropTarget>
    );
  },
);

export const EnhancedEditor = memo(EnhancedEditorInner);
