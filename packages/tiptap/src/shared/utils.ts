import { MarkdownManager } from "@tiptap/markdown";
import type { JSONContent } from "@tiptap/react";

import { getExtensions } from "./extensions";

export const EMPTY_TIPTAP_DOC: JSONContent = {
  type: "doc",
  content: [{ type: "paragraph" }],
};

let _markdownManager: MarkdownManager | null = null;

function getMarkdownManager(): MarkdownManager {
  if (!_markdownManager) {
    _markdownManager = new MarkdownManager({ extensions: getExtensions() });
  }
  return _markdownManager;
}

export function isValidTiptapContent(content: unknown): content is JSONContent {
  if (!content || typeof content !== "object") {
    return false;
  }

  const obj = content as Record<string, unknown>;
  return obj.type === "doc" && Array.isArray(obj.content);
}

export function parseJsonContent(raw: string | undefined | null): JSONContent {
  if (typeof raw !== "string" || !raw.trim()) {
    return EMPTY_TIPTAP_DOC;
  }

  try {
    const parsed = JSON.parse(raw);
    return isValidTiptapContent(parsed) ? parsed : EMPTY_TIPTAP_DOC;
  } catch {
    return EMPTY_TIPTAP_DOC;
  }
}

export function json2md(jsonContent: JSONContent): string {
  return getMarkdownManager().serialize(jsonContent);
}

export function md2json(markdown: string): JSONContent {
  try {
    const json = getMarkdownManager().parse(markdown);
    return sanitizeMarks(json);
  } catch (error) {
    console.error(error);

    return {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: markdown }],
        },
      ],
    };
  }
}

/**
 * The `code` mark has `excludes: "_"` in TipTap, meaning it excludes all other marks.
 * When `code` is present on a text node, strip all other marks to match ProseMirror's schema.
 */
function sanitizeMarks(node: JSONContent): JSONContent {
  if (node.type === "text" && node.marks) {
    const hasCode = node.marks.some((m) => m.type === "code");
    if (hasCode && node.marks.length > 1) {
      return { ...node, marks: [{ type: "code" }] };
    }
    return node;
  }

  if (node.content) {
    return { ...node, content: node.content.map(sanitizeMarks) };
  }

  return node;
}
