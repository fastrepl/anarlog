import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import type {
  JSONContent,
  SlashCommandConfig,
  TiptapEditor,
} from "@hypr/tiptap/chat";

import {
  clearDraftContent,
  getDraftContent,
  serializeDraftMessage,
  setDraftContent,
} from "./draft";

import type { ContextRef } from "~/chat/context/entities";
import { useSearchEngine } from "~/search/contexts/engine";
import * as main from "~/store/tinybase/store/main";

export function useDraftState({
  draftKey,
  onContextRefsChange,
}: {
  draftKey: string;
  onContextRefsChange?: (refs: ContextRef[]) => void;
}) {
  const initialContent = useRef(getDraftContent(draftKey));
  const [hasContent, setHasContent] = useState(() => {
    return serializeDraftMessage(initialContent.current).text.trim().length > 0;
  });

  const handleEditorUpdate = useCallback(
    (json: JSONContent) => {
      const draft = serializeDraftMessage(json);
      const text = draft.text.trim();
      setHasContent(text.length > 0);
      setDraftContent(draftKey, json);
      onContextRefsChange?.(draft.refs);
    },
    [draftKey, onContextRefsChange],
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
  onContextRefsChange,
}: {
  draftKey: string;
  editorRef: React.RefObject<{ editor: TiptapEditor | null } | null>;
  disabled?: boolean;
  isStreaming?: boolean;
  onSendMessage: (
    content: string,
    parts: Array<{ type: "text"; text: string }>,
    contextRefs?: ContextRef[],
  ) => void;
  onContextRefsChange?: (refs: ContextRef[]) => void;
}) {
  return useCallback(() => {
    const json = editorRef.current?.editor?.getJSON();
    const draft = serializeDraftMessage(json);
    const text = draft.text.trim();

    if (!text || disabled || isStreaming) {
      return;
    }

    void analyticsCommands.event({ event: "message_sent" });
    onSendMessage(text, [{ type: "text", text }], draft.refs);
    editorRef.current?.editor?.commands.clearContent();
    clearDraftContent(draftKey);
    onContextRefsChange?.([]);
  }, [
    draftKey,
    editorRef,
    disabled,
    isStreaming,
    onSendMessage,
    onContextRefsChange,
  ]);
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

export function useSyncDraftStateFromEditor({
  editorRef,
  handleEditorUpdate,
}: {
  editorRef: React.RefObject<{ editor: TiptapEditor | null } | null>;
  handleEditorUpdate: (json: JSONContent) => void;
}) {
  useEffect(() => {
    let rafId: number | null = null;
    let attempts = 0;
    const maxAttempts = 20;

    const syncWhenReady = () => {
      const editor = editorRef.current?.editor;

      if (editor && !editor.isDestroyed && editor.isInitialized) {
        handleEditorUpdate(editor.getJSON());
        return;
      }

      if (attempts >= maxAttempts) {
        return;
      }

      attempts += 1;
      rafId = window.requestAnimationFrame(syncWhenReady);
    };

    syncWhenReady();

    return () => {
      if (rafId !== null) {
        window.cancelAnimationFrame(rafId);
      }
    };
  }, [editorRef, handleEditorUpdate]);
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
