import { forwardRef, useMemo } from "react";

import { useMentionConfig } from "~/chat/hooks/mention-config";
import { parseJsonContent } from "~/editor/markdown";
import {
  NoteEditor,
  type JSONContent,
  type NoteEditorRef,
} from "~/editor/session";
import {
  useEnhancedNoteCell,
  useUpdateEnhancedNoteContent,
} from "~/session/hooks/enhanced-notes";
import { useFileUpload } from "~/shared/hooks/useFileUpload";

export const EnhancedEditor = forwardRef<
  NoteEditorRef,
  {
    sessionId: string;
    enhancedNoteId: string;
    onNavigateToTitle?: (pixelWidth?: number) => void;
  }
>(({ sessionId, enhancedNoteId, onNavigateToTitle }, ref) => {
  const onFileUpload = useFileUpload(sessionId);
  const content = useEnhancedNoteCell(enhancedNoteId, "content");

  const initialContent = useMemo<JSONContent>(
    () => parseJsonContent(content),
    [content],
  );

  const persistContent = useUpdateEnhancedNoteContent(enhancedNoteId);
  const handleChange = useMemo(
    () => (input: JSONContent) => persistContent(JSON.stringify(input)),
    [persistContent],
  );

  const mentionConfig = useMentionConfig();

  const fileHandlerConfig = useMemo(() => ({ onFileUpload }), [onFileUpload]);

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
      />
    </div>
  );
});
