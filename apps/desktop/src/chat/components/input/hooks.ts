import { useCallback, useEffect, useRef, useState } from "react";

import type { ChatEditorHandle, JSONContent } from "@anlg/editor/chat";
import { EMPTY_DOC } from "@anlg/editor/markdown";
import { commands as analyticsCommands } from "@anlg/plugin-analytics";

import { pushSentMessage, sentMessageAt, sentMessageCount } from "./history";

import type { ContextRef } from "~/chat/context/entities";

const draftsByKey = new Map<string, JSONContent>();

export function useDraftState({
  draftKey,
  onDraftContentChange,
  onContextRefsChange,
  onUserEdit,
  shouldPersistUpdate,
}: {
  draftKey: string;
  onDraftContentChange?: (hasDraftContent: boolean) => void;
  onContextRefsChange?: (refs: ContextRef[]) => void;
  onUserEdit?: () => void;
  shouldPersistUpdate?: () => boolean;
}) {
  const initialContent = useRef(draftsByKey.get(draftKey) ?? EMPTY_DOC);
  const [hasContent, setHasContent] = useState(() =>
    hasTextContent(initialContent.current),
  );

  useEffect(() => {
    onDraftContentChange?.(hasDraftContent(initialContent.current));
    onContextRefsChange?.(
      extractContextRefsFromTiptapJson(initialContent.current),
    );
  }, [onDraftContentChange, onContextRefsChange]);

  const handleEditorUpdate = useCallback(
    (json: JSONContent) => {
      setHasContent(hasTextContent(json));
      const shouldPersist = shouldPersistUpdate?.() ?? true;
      if (shouldPersist) {
        draftsByKey.set(draftKey, json);
      }
      onDraftContentChange?.(hasDraftContent(json));
      onContextRefsChange?.(extractContextRefsFromTiptapJson(json));
      if (shouldPersist) {
        onUserEdit?.();
      }
    },
    [
      draftKey,
      onDraftContentChange,
      onContextRefsChange,
      onUserEdit,
      shouldPersistUpdate,
    ],
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
  onSendMessage,
  onDraftContentChange,
  onContextRefsChange,
  onSubmitted,
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
  onDraftContentChange?: (hasDraftContent: boolean) => void;
  onContextRefsChange?: (refs: ContextRef[]) => void;
  onSubmitted?: (json: JSONContent | undefined) => void;
}) {
  return useCallback(() => {
    const json = editorRef.current?.getJSON();
    const text = proseMirrorJsonToText(json).trim();
    const mentionRefs = extractContextRefsFromTiptapJson(json);

    if (!text || disabled) {
      return;
    }

    void analyticsCommands.event({ event: "message_sent" });
    onSendMessage(text, [{ type: "text", text }], mentionRefs);
    editorRef.current?.clearContent();
    draftsByKey.delete(draftKey);
    onDraftContentChange?.(false);
    onContextRefsChange?.([]);
    onSubmitted?.(json);
  }, [
    draftKey,
    editorRef,
    disabled,
    onSendMessage,
    onDraftContentChange,
    onContextRefsChange,
    onSubmitted,
  ]);
}

export function useMessageHistory({
  editorRef,
}: {
  editorRef: React.RefObject<ChatEditorHandle | null>;
}) {
  const [index, setIndex] = useState<number | null>(null);
  const draftBeforeHistory = useRef<JSONContent | undefined>(undefined);
  const isApplyingRef = useRef(false);

  const applyContent = useCallback(
    (content: JSONContent | undefined, selection: "start" | "end") => {
      isApplyingRef.current = true;
      editorRef.current?.replaceContent(content ?? EMPTY_DOC, selection);
      isApplyingRef.current = false;
    },
    [editorRef],
  );

  const navigate = useCallback(
    (direction: "prev" | "next") => {
      if (direction === "prev") {
        const total = sentMessageCount();
        if (total === 0) {
          return false;
        }

        const next = index === null ? 0 : index + 1;
        if (next >= total) {
          return true;
        }

        if (index === null) {
          draftBeforeHistory.current = editorRef.current?.getJSON();
        }
        setIndex(next);
        applyContent(sentMessageAt(next), "start");
        return true;
      }

      if (index === null) {
        return false;
      }

      if (index === 0) {
        setIndex(null);
        applyContent(draftBeforeHistory.current, "end");
        return true;
      }

      setIndex(index - 1);
      applyContent(sentMessageAt(index - 1), "end");
      return true;
    },
    [applyContent, editorRef, index],
  );

  const handleUserEdit = useCallback(() => {
    if (isApplyingRef.current) {
      return;
    }
    setIndex(null);
  }, []);

  const shouldPersistUpdate = useCallback(() => !isApplyingRef.current, []);

  const handleSubmitted = useCallback((json: JSONContent | undefined) => {
    pushSentMessage(json);
    draftBeforeHistory.current = undefined;
    setIndex(null);
  }, []);

  return {
    position: index === null ? null : index + 1,
    total: sentMessageCount(),
    navigate,
    handleUserEdit,
    handleSubmitted,
    shouldPersistUpdate,
  };
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

function proseMirrorJsonToText(json: any): string {
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
    return json.content.map(proseMirrorJsonToText).join("");
  }

  return "";
}

function hasTextContent(json: JSONContent | undefined): boolean {
  return proseMirrorJsonToText(json).trim().length > 0;
}

function hasDraftContent(json: JSONContent | undefined): boolean {
  if (hasTextContent(json)) {
    return true;
  }

  return hasAttachmentNode(json);
}

function hasAttachmentNode(json: JSONContent | undefined): boolean {
  if (!json || typeof json !== "object") {
    return false;
  }

  if (json.type === "attachment") {
    return true;
  }

  return Array.isArray(json.content) && json.content.some(hasAttachmentNode);
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
