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
import { SessionNodeView } from "~/editor-bridge/session-view";
import { useFileUpload } from "~/shared/hooks/useFileUpload";
import * as main from "~/store/tinybase/store/main";

const extraNodeViews = { appLink: AppLinkView, session: SessionNodeView };

export const RawEditor = forwardRef<
  NoteEditorRef,
  {
    sessionId: string;
    onNavigateToTitle?: (pixelWidth?: number) => void;
  }
>(({ sessionId, onNavigateToTitle }, ref) => {
  const rawMd = main.UI.useCell("sessions", sessionId, "raw_md", main.STORE_ID);
  const onFileUpload = useFileUpload(sessionId);

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
      persistChange(input);

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
    [persistChange, hasNonEmptyText],
  );

  const fileHandlerConfig = useMemo(() => ({ onFileUpload }), [onFileUpload]);
  const mentionConfig = useMentionConfig();

  return (
    <NoteEditor
      ref={ref}
      key={`session-${sessionId}-raw`}
      initialContent={initialContent}
      handleChange={handleChange}
      mentionConfig={mentionConfig}
      placeholderComponent={Placeholder}
      onNavigateToTitle={onNavigateToTitle}
      fileHandlerConfig={fileHandlerConfig}
      taskSource={{ type: "session_raw_note", id: sessionId }}
      extraNodeViews={extraNodeViews}
    />
  );
});

const Placeholder: PlaceholderFunction = ({ node, pos }) => {
  if (node.type.name !== "paragraph") {
    return "";
  }

  if (pos === 0) {
    return "Take notes to guide Char's meeting notes. Press / for commands.";
  }

  return "Press / for commands.";
};
