import type { EditorView } from "prosemirror-view";
import { forwardRef, useCallback, useEffect, useMemo, useRef } from "react";

import { parseJsonContent } from "@hypr/editor/markdown";
import {
  NoteEditor,
  type JSONContent,
  type NoteEditorRef,
  type PlaceholderFunction,
} from "@hypr/editor/note";
import { commands as analyticsCommands } from "@hypr/plugin-analytics";

import { AppLinkView } from "~/editor-bridge/app-link-view";
import { useMentionConfig } from "~/editor-bridge/mention-config";
import { openEditorLink } from "~/editor-bridge/open-editor-link";
import { sessionMentionDropConfig } from "~/editor-bridge/session-mention-drop";
import { SessionNodeView } from "~/editor-bridge/session-view";
import { emitRawEditorSync } from "~/session/raw-editor-sync";
import { useFileUpload } from "~/shared/hooks/useFileUpload";
import * as main from "~/store/tinybase/store/main";

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
    const rawMd = main.UI.useCell(
      "sessions",
      sessionId,
      "raw_md",
      main.STORE_ID,
    );
    const onFileUpload = useFileUpload(sessionId);
    const syncSourceId = useRawEditorSyncSourceId();

    const initialContent = useMemo<JSONContent>(
      () => parseJsonContent(rawMd as string),
      [rawMd],
    );

    const persistChange = main.UI.useSetPartialRowCallback(
      "sessions",
      sessionId,
      (input: JSONContent) => ({ raw_md: JSON.stringify(input) }),
      [],
      main.STORE_ID,
    );

    const hasTrackedWriteRef = useRef(false);

    useEffect(() => {
      hasTrackedWriteRef.current = false;
    }, [sessionId]);

    const hasNonEmptyText = useCallback(
      (node?: JSONContent): boolean =>
        !!node?.text?.trim() ||
        !!node?.content?.some((child: JSONContent) => hasNonEmptyText(child)),
      [],
    );

    const handleChange = useCallback(
      (input: JSONContent) => {
        const nextRawMd = JSON.stringify(input);
        persistChange(input);
        emitRawEditorSync({
          sessionId,
          rawMd: nextRawMd,
          sourceId: syncSourceId,
        });

        if (!hasTrackedWriteRef.current) {
          const hasContent = hasNonEmptyText(input);
          if (hasContent) {
            hasTrackedWriteRef.current = true;
            void analyticsCommands.event({
              event: "note_edited",
              has_content: true,
            });
          }
        }
      },
      [persistChange, sessionId, syncSourceId, hasNonEmptyText],
    );

    const fileHandlerConfig = useMemo(() => ({ onFileUpload }), [onFileUpload]);
    const mentionConfig = useMentionConfig();

    return (
      <NoteEditor
        ref={ref}
        className={className}
        key={`session-${sessionId}-raw`}
        initialContent={initialContent}
        handleChange={handleChange}
        mentionConfig={mentionConfig}
        sessionMentionDropConfig={sessionMentionDropConfig}
        placeholderComponent={Placeholder}
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
    );
  },
);

function useRawEditorSyncSourceId() {
  const sourceIdRef = useRef<string | null>(null);
  if (!sourceIdRef.current) {
    sourceIdRef.current = crypto.randomUUID();
  }

  return sourceIdRef.current;
}

const Placeholder: PlaceholderFunction = ({ node, pos }) => {
  if (node.type.name !== "paragraph") {
    return "";
  }

  if (pos === 0) {
    return "Take notes to guide Anarlog's meeting notes. Press / for commands.";
  }

  return "Press / for commands.";
};
