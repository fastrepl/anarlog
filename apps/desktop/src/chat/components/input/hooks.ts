import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import type {
  JSONContent,
  SlashCommandConfig,
  TiptapEditor,
} from "@hypr/tiptap/chat";
import { EMPTY_TIPTAP_DOC } from "@hypr/tiptap/shared";

import type { ContextRef } from "~/chat/context/entities";
import { useSearchEngine } from "~/search/contexts/engine";
import * as main from "~/store/tinybase/store/main";

const draftsByKey = new Map<string, JSONContent>();

export function useDraftState({
  draftKey,
  onContextRefsChange,
}: {
  draftKey: string;
  onContextRefsChange?: (refs: ContextRef[]) => void;
}) {
  const [hasContent, setHasContent] = useState(false);
  const initialContent = useRef(draftsByKey.get(draftKey) ?? EMPTY_TIPTAP_DOC);

  useEffect(() => {
    onContextRefsChange?.(serializeDraftMessage(initialContent.current).refs);
  }, [onContextRefsChange]);

  const handleEditorUpdate = useCallback(
    (json: JSONContent) => {
      const draft = serializeDraftMessage(json);
      const text = draft.text.trim();
      setHasContent(text.length > 0);
      draftsByKey.set(draftKey, json);
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
    draftsByKey.delete(draftKey);
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

export function serializeDraftMessage(json: JSONContent | undefined): {
  text: string;
  refs: ContextRef[];
} {
  const textParts: string[] = [];
  const refs: ContextRef[] = [];
  const seen = new Set<string>();

  const visit = (node: JSONContent | undefined) => {
    if (!node || typeof node !== "object") {
      return;
    }

    if (node.type === "text") {
      textParts.push(node.text || "");
      return;
    }

    if (node.type === "hardBreak") {
      textParts.push("\n");
      return;
    }

    if (isMentionNode(node)) {
      textParts.push(mentionNodeToPlainText(node));

      const mentionType =
        typeof node.attrs?.type === "string" ? node.attrs.type : null;
      const mentionId =
        typeof node.attrs?.id === "string" ? node.attrs.id : null;

      if (!mentionType || !mentionId) {
        return;
      }

      let ref: ContextRef | null = null;
      if (mentionType === "session") {
        ref = {
          kind: "session",
          key: `session:manual:${mentionId}`,
          label:
            typeof node.attrs?.label === "string"
              ? node.attrs.label
              : undefined,
          source: "draft",
          sessionId: mentionId,
        };
      } else if (mentionType === "human") {
        ref = {
          kind: "human",
          key: `human:manual:${mentionId}`,
          label:
            typeof node.attrs?.label === "string"
              ? node.attrs.label
              : undefined,
          source: "draft",
          humanId: mentionId,
        };
      } else if (mentionType === "organization") {
        ref = {
          kind: "organization",
          key: `organization:manual:${mentionId}`,
          label:
            typeof node.attrs?.label === "string"
              ? node.attrs.label
              : undefined,
          source: "draft",
          organizationId: mentionId,
        };
      }

      if (ref && !seen.has(ref.key)) {
        seen.add(ref.key);
        refs.push(ref);
      }

      return;
    }

    if (Array.isArray(node.content)) {
      for (const child of node.content) {
        visit(child);
      }
    }
  };

  visit(json);
  return { text: textParts.join(""), refs };
}

function isMentionNode(
  node: Pick<JSONContent, "type" | "attrs"> | Record<string, unknown>,
): boolean {
  return (
    typeof node.type === "string" &&
    (node.type === "mention" || node.type.startsWith("mention-"))
  );
}

function mentionNodeToPlainText(node: JSONContent): string {
  const label =
    typeof node.attrs?.label === "string" && node.attrs.label.trim()
      ? node.attrs.label.trim()
      : typeof node.attrs?.id === "string" && node.attrs.id.trim()
        ? node.attrs.id.trim()
        : "";

  return label ? `@${label}` : "";
}
