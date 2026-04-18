import { forwardRef, useCallback, useEffect, useMemo, useRef } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";

import { useMentionConfig } from "~/chat/hooks/mention-config";
import { parseJsonContent } from "~/editor/markdown";
import {
  NoteEditor,
  type JSONContent,
  type NoteEditorRef,
  type PlaceholderFunction,
} from "~/editor/session";
import { useSessionCell, useUpdateSessionCell } from "~/session/hooks/sessions";
import { useFileUpload } from "~/shared/hooks/useFileUpload";

export const RawEditor = forwardRef<
  NoteEditorRef,
  {
    sessionId: string;
    onNavigateToTitle?: (pixelWidth?: number) => void;
  }
>(({ sessionId, onNavigateToTitle }, ref) => {
  const rawMd = useSessionCell(sessionId, "raw_md");
  const onFileUpload = useFileUpload(sessionId);

  const initialContent = useMemo<JSONContent>(
    () => parseJsonContent(rawMd),
    [rawMd],
  );

  const setRawMd = useUpdateSessionCell(sessionId, "raw_md");
  const persistChange = useCallback(
    (input: JSONContent) => {
      setRawMd(JSON.stringify(input));
    },
    [setRawMd],
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

  const mentionConfig = useMentionConfig();

  const fileHandlerConfig = useMemo(() => ({ onFileUpload }), [onFileUpload]);

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
