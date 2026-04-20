import { useCallback, useEffect, useRef, useState } from "react";

import type { ChatEditorHandle, JSONContent } from "@hypr/editor/chat";
import { EMPTY_DOC } from "@hypr/editor/markdown";
import { commands as analyticsCommands } from "@hypr/plugin-analytics";

import type { ContextRef } from "~/chat/context/entities";

const draftsByKey = new Map<string, JSONContent>();

export function useDraftState({
  draftKey,
  onContextRefsChange,
}: {
  draftKey: string;
  onContextRefsChange?: (refs: ContextRef[]) => void;
}) {
  const [hasContent, setHasContent] = useState(false);
  const initialContent = useRef(draftsByKey.get(draftKey) ?? EMPTY_DOC);

  useEffect(() => {
    onContextRefsChange?.(
      extractContextRefsFromTiptapJson(initialContent.current),
    );
  }, [onContextRefsChange]);

  const handleEditorUpdate = useCallback(
    (json: JSONContent) => {
      const text = tiptapJsonToText(json).trim();
      setHasContent(text.length > 0);
      draftsByKey.set(draftKey, json);
      onContextRefsChange?.(extractContextRefsFromTiptapJson(json));
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
  editorRef: React.RefObject<ChatEditorHandle | null>;
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
    const json = editorRef.current?.getJSON();
    const text = tiptapJsonToText(json).trim();
    const mentionRefs = extractContextRefsFromTiptapJson(json);

    if (!text || disabled || isStreaming) {
      return;
    }

    void analyticsCommands.event({ event: "message_sent" });
    onSendMessage(text, [{ type: "text", text }], mentionRefs);
    editorRef.current?.clearContent();
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
  editorRef: React.RefObject<ChatEditorHandle | null>;
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

    const tryFocus = () => {
      if (editorRef.current) {
        editorRef.current.focus();
        return;
      }

      if (attempts >= maxAttempts) {
        return;
      }

      attempts += 1;
      rafId = window.requestAnimationFrame(tryFocus);
    };

    tryFocus();

    return () => {
      if (rafId !== null) {
        window.cancelAnimationFrame(rafId);
      }
    };
  }, [editorRef, disabled, shouldFocus]);
}

function tiptapJsonToText(json: any): string {
  if (!json || typeof json !== "object") {
    return "";
  }

  if (json.type === "text") {
    return json.text || "";
  }

  if (typeof json.type === "string" && json.type.startsWith("mention-")) {
    return `@${json.attrs?.label || json.attrs?.id || ""}`;
  }

  if (json.content && Array.isArray(json.content)) {
    return json.content.map(tiptapJsonToText).join("");
  }

  return "";
}

function extractContextRefsFromTiptapJson(
  json: JSONContent | undefined,
): ContextRef[] {
  const refs: ContextRef[] = [];
  const seen = new Set<string>();

  const visit = (node: JSONContent | undefined) => {
    if (!node || typeof node !== "object") {
      return;
    }

    if (typeof node.type === "string" && node.type.startsWith("mention-")) {
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
          source: "manual",
          sessionId: mentionId,
        };
      } else if (mentionType === "human") {
        ref = {
          kind: "human",
          key: `human:manual:${mentionId}`,
          source: "manual",
          humanId: mentionId,
        };
      } else if (mentionType === "organization") {
        ref = {
          kind: "organization",
          key: `organization:manual:${mentionId}`,
          source: "manual",
          organizationId: mentionId,
        };
      }

      if (ref && !seen.has(ref.key)) {
        seen.add(ref.key);
        refs.push(ref);
      }
    }

    if (Array.isArray(node.content)) {
      for (const child of node.content) {
        visit(child);
      }
    }
  };

  visit(json);
  return refs;
}
