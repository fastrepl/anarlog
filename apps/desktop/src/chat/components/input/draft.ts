import type { JSONContent } from "@hypr/tiptap/chat";
import { EMPTY_TIPTAP_DOC } from "@hypr/tiptap/shared";

import type { ContextRef } from "~/chat/context/entities";

const draftsByKey = new Map<string, JSONContent>();

export function getDraftContent(draftKey: string): JSONContent {
  return draftsByKey.get(draftKey) ?? EMPTY_TIPTAP_DOC;
}

export function setDraftContent(draftKey: string, content: JSONContent) {
  draftsByKey.set(draftKey, content);
}

export function clearDraftContent(draftKey: string) {
  draftsByKey.delete(draftKey);
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

export function getDraftContextRefs(draftKey: string): ContextRef[] {
  return serializeDraftMessage(getDraftContent(draftKey)).refs;
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
