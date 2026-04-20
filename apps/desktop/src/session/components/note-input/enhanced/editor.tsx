import { forwardRef, useMemo } from "react";

import { parseJsonContent } from "@hypr/editor/markdown";
import {
  NoteEditor,
  type JSONContent,
  type NoteEditorRef,
} from "@hypr/editor/note";

import { AppLinkView } from "~/editor-bridge/app-link-view";
import { useMentionConfig } from "~/editor-bridge/mention-config";
import { SessionNodeView } from "~/editor-bridge/session-view";
import { useFileUpload } from "~/shared/hooks/useFileUpload";
import * as main from "~/store/tinybase/store/main";

const extraNodeViews = { appLink: AppLinkView, session: SessionNodeView };

export const EnhancedEditor = forwardRef<
  NoteEditorRef,
  {
    sessionId: string;
    enhancedNoteId: string;
    onNavigateToTitle?: (pixelWidth?: number) => void;
  }
>(({ sessionId, enhancedNoteId, onNavigateToTitle }, ref) => {
  const onFileUpload = useFileUpload(sessionId);
  const content = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId,
    "content",
    main.STORE_ID,
  );

  const initialContent = useMemo<JSONContent>(
    () => parseJsonContent(content as string),
    [content],
  );

  const handleChange = main.UI.useSetPartialRowCallback(
    "enhanced_notes",
    enhancedNoteId,
    (input: JSONContent) => ({ content: JSON.stringify(input) }),
    [],
    main.STORE_ID,
  );

  const fileHandlerConfig = useMemo(() => ({ onFileUpload }), [onFileUpload]);
  const mentionConfig = useMentionConfig();

  return (
    <div className="h-full">
      <NoteEditor
        ref={ref}
        key={`enhanced-note-${enhancedNoteId}`}
        initialContent={initialContent}
        handleChange={handleChange}
        mentionConfig={mentionConfig}
        onNavigateToTitle={onNavigateToTitle}
        fileHandlerConfig={fileHandlerConfig}
        taskSource={{ type: "enhanced_note", id: enhancedNoteId }}
        extraNodeViews={extraNodeViews}
      />
    </div>
  );
});
