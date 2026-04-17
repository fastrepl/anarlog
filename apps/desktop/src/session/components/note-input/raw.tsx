import { forwardRef, useCallback, useEffect, useMemo, useRef } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";

import { parseJsonContent } from "~/editor/markdown";
import {
  NoteEditor,
  type JSONContent,
  type NoteEditorRef,
  type PlaceholderFunction,
} from "~/editor/session";
import { useSearchEngine } from "~/search/contexts/engine";
import { useFileUpload } from "~/shared/hooks/useFileUpload";
import * as main from "~/store/tinybase/store/main";

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
