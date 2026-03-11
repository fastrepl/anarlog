import { forwardRef, useCallback, useEffect, useMemo, useRef } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import NoteEditor, {
  type JSONContent,
  type TiptapEditor,
} from "@hypr/tiptap/editor";
import {
  parseJsonContent,
  type PlaceholderFunction,
} from "@hypr/tiptap/shared";

import { useSearchEngine } from "~/search/contexts/engine";
import { useImageUpload } from "~/shared/hooks/useImageUpload";
import * as main from "~/store/tinybase/store/main";

export const RawEditor = forwardRef<
  { editor: TiptapEditor | null },
  { sessionId: string; onNavigateToTitle?: () => void }
>(({ sessionId, onNavigateToTitle }, ref) => {
  const rawMd = main.UI.useCell("sessions", sessionId, "raw_md", main.STORE_ID);
  const onImageUpload = useImageUpload(sessionId);

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
    <NoteEditor
      ref={ref}
      key={`session-${sessionId}-raw`}
      initialContent={initialContent}
      handleChange={handleChange}
      mentionConfig={mentionConfig}
      placeholderComponent={Placeholder}
      onNavigateToTitle={onNavigateToTitle}
      fileHandlerConfig={fileHandlerConfig}
      extensionOptions={extensionOptions}
    />
  );
});

const Placeholder: PlaceholderFunction = ({ node, pos }) => {
  "use no memo";
  if (node.type.name === "paragraph" && pos === 0) {
    return (
      <p className="text-[#e5e5e5]">
        Take notes or press <kbd>/</kbd> for commands.
      </p>
    );
  }

  return "";
};
