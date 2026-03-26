import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import type {
  JSONContent,
  SlashCommandConfig,
  TiptapEditor,
} from "@hypr/tiptap/chat";
import { EMPTY_TIPTAP_DOC } from "@hypr/tiptap/shared";

import { useSearchEngine } from "~/search/contexts/engine";
import * as main from "~/store/tinybase/store/main";

const draftsByKey = new Map<string, JSONContent>();

export function useDraftState({ draftKey }: { draftKey: string }) {
  const [hasContent, setHasContent] = useState(false);
  const initialContent = useRef(draftsByKey.get(draftKey) ?? EMPTY_TIPTAP_DOC);

  const handleEditorUpdate = useCallback(
    (json: JSONContent) => {
      const text = tiptapJsonToText(json).trim();
      setHasContent(text.length > 0);
      draftsByKey.set(draftKey, json);
    },
    [draftKey],
  );

  return {
    hasContent,
    initialContent: initialContent.current,
    handleEditorUpdate,
  };
}

export function useSubmit({
  draftKey,
  editorRef,
  disabled,
  isStreaming,
  onSendMessage,
}: {
  draftKey: string;
  editorRef: React.RefObject<{ editor: TiptapEditor | null } | null>;
  disabled?: boolean;
  isStreaming?: boolean;
  onSendMessage: (
    content: string,
    parts: Array<{ type: "text"; text: string }>,
  ) => void;
}) {
  return useCallback(() => {
    const json = editorRef.current?.editor?.getJSON();
    const text = tiptapJsonToText(json).trim();

    if (!text || disabled || isStreaming) {
      return;
    }

    void analyticsCommands.event({ event: "message_sent" });
    onSendMessage(text, [{ type: "text", text }]);
    editorRef.current?.editor?.commands.clearContent();
    draftsByKey.delete(draftKey);
  }, [draftKey, editorRef, disabled, isStreaming, onSendMessage]);
}

export function useAutoFocusEditor({
  editorRef,
  disabled,
  shouldFocus = true,
}: {
  editorRef: React.RefObject<{ editor: TiptapEditor | null } | null>;
  disabled?: boolean;
  shouldFocus?: boolean;
}) {
  useEffect(() => {
    if (disabled || !shouldFocus) {
      return;
    }

    let rafId: number | null = null;
    let attempts = 0;
    const maxAttempts = 20;

    const focusWhenReady = () => {
      const editor = editorRef.current?.editor;

      if (editor && !editor.isDestroyed && editor.isInitialized) {
        editor.commands.focus();
        return;
      }

      if (attempts >= maxAttempts) {
        return;
      }

      attempts += 1;
      rafId = window.requestAnimationFrame(focusWhenReady);
    };

    focusWhenReady();

    return () => {
      if (rafId !== null) {
        window.cancelAnimationFrame(rafId);
      }
    };
  }, [editorRef, disabled, shouldFocus]);
}

export function useSlashCommandConfig(): SlashCommandConfig {
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
  const { search } = useSearchEngine();

  return useMemo(
    () => ({
      handleSearch: async (query: string) => {
        const results: {
          id: string;
          type: string;
          label: string;
          content?: string;
        }[] = [];

        if (query.trim()) {
          const searchResults = await search(query);
          for (const hit of searchResults) {
            results.push({
              id: hit.document.id,
              type: hit.document.type,
              label: hit.document.title,
            });
          }
        } else {
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
        }

        return results.slice(0, 5);
      },
    }),
    [sessions, humans, organizations, search],
  );
}

function tiptapJsonToText(json: any): string {
  if (!json || typeof json !== "object") {
    return "";
  }

  if (json.type === "text") {
    return json.text || "";
  }

  if (json.type === "mention") {
    return `@${json.attrs?.label || json.attrs?.id || ""}`;
  }

  if (json.content && Array.isArray(json.content)) {
    return json.content.map(tiptapJsonToText).join("");
  }

  return "";
}
