import { forwardRef, useMemo } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { type JSONContent, TiptapEditor } from "@hypr/tiptap/editor";
import NoteEditor from "@hypr/tiptap/editor";
import { parseJsonContent } from "@hypr/tiptap/shared";

import { useSearchEngine } from "~/search/contexts/engine";
import { useImageUpload } from "~/shared/hooks/useImageUpload";
import * as main from "~/store/tinybase/store/main";

export const EnhancedEditor = forwardRef<
  { editor: TiptapEditor | null },
  { sessionId: string; enhancedNoteId: string; onNavigateToTitle?: () => void }
>(({ sessionId, enhancedNoteId, onNavigateToTitle }, ref) => {
  const onImageUpload = useImageUpload(sessionId);
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

  const fileHandlerConfig = useMemo(() => ({ onImageUpload }), [onImageUpload]);

  const extensionOptions = useMemo(
    () => ({
      onLinkOpen: (url: string) => {
        void openerCommands.openUrl(url, null);
      },
    }),
    [],
  );

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
        extensionOptions={extensionOptions}
      />
    </div>
  );
});
