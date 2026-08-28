import * as stylex from "@stylexjs/stylex";
import type { EditorView } from "prosemirror-view";
import { forwardRef, memo, useCallback, useMemo } from "react";

import { parseJsonContent } from "@anlg/editor/markdown";
import {
  NoteEditor,
  type JSONContent,
  type NoteEditorRef,
  normalizePortableAttachmentUrls,
} from "@anlg/editor/note";

import { AudioDropTarget } from "../audio-drop-target";
import { useNoteFileHandlerConfig } from "../file-handler";

import { AppLinkView } from "~/editor-bridge/app-link-view";
import { useMentionConfig } from "~/editor-bridge/mention-config";
import { openEditorLink } from "~/editor-bridge/open-editor-link";
import { sessionMentionDropConfig } from "~/editor-bridge/session-mention-drop";
import { SessionNodeView } from "~/editor-bridge/session-view";
import {
  SessionCommentsLayer,
  useOwnedSessionComments,
} from "~/session-sharing/comments";
import { hasStoredNoteContent } from "~/session/components/shared";
import { useAttachmentResolver } from "~/session/hooks/useAttachmentResolver";
import { useUpdateEnhancedNoteContent } from "~/session/queries";
import {
  ensureFirstLineTitle,
  extractFirstLineTitle,
  documentTitlePlaceholder,
} from "~/session/title-content";

const extraNodeViews = { appLink: AppLinkView, session: SessionNodeView };

function isCanonicalEmptyDocument(
  content: JSONContent,
  sessionTitle: string,
): boolean {
  const [title, body, ...rest] = content.content ?? [];
  const expectedTitle = sessionTitle.trim();
  const titleContent = title?.content ?? [];
  const titleAttrs = title?.attrs ?? {};
  const bodyAttrs = body?.attrs ?? {};
  const hasExpectedTitle = expectedTitle
    ? titleContent.length === 1 &&
      titleContent[0]?.type === "text" &&
      titleContent[0].text === expectedTitle &&
      !titleContent[0].marks?.length
    : titleContent.length === 0;
  return (
    content.type === "doc" &&
    rest.length === 0 &&
    title?.type === "heading" &&
    titleAttrs.level === 1 &&
    Object.keys(titleAttrs).length === 1 &&
    hasExpectedTitle &&
    body?.type === "paragraph" &&
    Object.keys(bodyAttrs).length === 0 &&
    !body.content?.length
  );
}

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
        if (
          content === "" &&
          isCanonicalEmptyDocument(portableInput, sessionTitle)
        ) {
          return;
        }
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
      [content, sessionTitle, updateContent],
    );

    const mentionConfig = useMentionConfig();
    const comments = useOwnedSessionComments(sessionId);
    // Stable identity: NoteEditor keys whole-document memos off this.
    const taskSource = useMemo(
      () =>
        persistChanges
          ? ({ type: "enhanced_note", id: enhancedNoteId } as const)
          : undefined,
      [persistChanges, enhancedNoteId],
    );

    return (
      <AudioDropTarget
        sx={styles.fullHeight}
        targetProps={audioDropTargetProps}
        isActive={isAudioDragActive}
      >
        <div ref={comments.containerRef} {...stylex.props(styles.container)}>
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
            taskSource={taskSource}
            extraNodeViews={extraNodeViews}
            commentAnchorsEnabled
            onCommentAnchorsEvent={comments.onCommentAnchorsEvent}
            onCommentSelection={
              comments.selection && !comments.draft
                ? comments.startDraft
                : undefined
            }
            onViewReady={(view) => {
              comments.onViewReady(view);
              onViewReady?.(view);
            }}
            onViewDisposed={(view) => {
              comments.onViewDisposed(view);
              onViewDisposed?.(view);
            }}
            syncContentWhenFocused={!persistChanges}
          />
          <SessionCommentsLayer controller={comments} />
        </div>
      </AudioDropTarget>
    );
  },
);

export const EnhancedEditor = memo(EnhancedEditorInner);

const styles = stylex.create({
  container: {
    height: "100%",
    position: "relative",
  },
  fullHeight: {
    height: "100%",
  },
});
