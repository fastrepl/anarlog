import { forwardRef, useMemo } from "react";

import { parseJsonContent } from "~/editor/markdown";
import {
  NoteEditor,
  type JSONContent,
  type NoteEditorRef,
} from "~/editor/session";
import { useSearchEngine } from "~/search/contexts/engine";
import { useFileUpload } from "~/shared/hooks/useFileUpload";
import * as main from "~/store/tinybase/store/main";

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

  const { search } = useSearchEngine();
  const sessions = main.UI.useResultTable(
    main.QUERIES.timelineSessions,
    main.STORE_ID,
  );
  const humans = main.UI.useResultTable(
    main.QUERIES.visibleHumans,
    main.STORE_ID,
  );
  const organizations = main.UI.useResultTable(
    main.QUERIES.visibleOrganizations,
    main.STORE_ID,
  );

  const mentionConfig = useMemo(
    () => ({
      trigger: "@",
      handleSearch: async (query: string) => {
        if (query.trim()) {
          const results = await search(query);
          return results.slice(0, 5).map((hit) => ({
            id: hit.document.id,
            type: hit.document.type,
            label: hit.document.title,
          }));
        }

        const results: { id: string; type: string; label: string }[] = [];
        Object.entries(sessions).forEach(([rowId, row]) => {
          const title = row.title as string | undefined;
          if (title) {
            results.push({ id: rowId, type: "session", label: title });
          }
        });
        Object.entries(humans).forEach(([rowId, row]) => {
          const name = row.name as string | undefined;
          if (name) {
            results.push({ id: rowId, type: "human", label: name });
          }
        });
        Object.entries(organizations).forEach(([rowId, row]) => {
          const name = row.name as string | undefined;
          if (name) {
            results.push({ id: rowId, type: "organization", label: name });
          }
        });
        return results.slice(0, 5);
      },
    }),
    [search, sessions, humans, organizations],
  );

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
