import { MarkdownManager } from "@tiptap/markdown";
import type { JSONContent } from "@tiptap/react";

import { getExtensions } from "./extensions";
import { validateJsonContent } from "./schema-validation";

export const EMPTY_TIPTAP_DOC: JSONContent = {
  type: "doc",
  content: [{ type: "paragraph" }],
};

let _markdownManager: MarkdownManager | null = null;

function hasDocShape(content: unknown): content is JSONContent {
  if (!content || typeof content !== "object") {
    return false;
  }

  const obj = content as Record<string, unknown>;
  return obj.type === "doc";
}

function getMarkdownManager(): MarkdownManager {
  if (!_markdownManager) {
    _markdownManager = new MarkdownManager({ extensions: getExtensions() });
  }
  return _markdownManager;
}

export function isValidTiptapContent(content: unknown): content is JSONContent {
  return hasDocShape(content) && validateJsonContent(content).valid;
}

export function parseJsonContent(raw: string | undefined | null): JSONContent {
  if (typeof raw !== "string" || !raw.trim()) {
    return EMPTY_TIPTAP_DOC;
  }

  try {
    const parsed = JSON.parse(raw);
    return hasDocShape(parsed) ? ensureValidContent(parsed) : EMPTY_TIPTAP_DOC;
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
    return ensureValidContent(sanitizeMarks(json), markdown);
  } catch (error) {
    console.error(error);

    return createFallbackDoc(markdown);
  }
}

function ensureValidContent(
  content: JSONContent,
  fallbackText?: string,
): JSONContent {
  const normalized = normalizeTiptapContent(content);
  const validation = validateJsonContent(normalized);

  if (validation.valid) {
    return normalized;
  }

  console.error("Failed to normalize TipTap content:", validation.error);
  return createFallbackDoc(fallbackText);
}

function normalizeTiptapContent(content: JSONContent): JSONContent {
  if (content.type !== "doc") {
    return EMPTY_TIPTAP_DOC;
  }

  return {
    ...content,
    content: normalizeBlockContainerContent(content.content),
  };
}

function normalizeBlockContainerContent(
  content: JSONContent[] | undefined,
): JSONContent[] {
  const blocks = (content ?? []).flatMap(normalizeBlockNode);
  return blocks.length > 0 ? blocks : [createParagraph()];
}

function normalizeBlockNode(node: JSONContent): JSONContent[] {
  switch (node.type) {
    case "paragraph":
      return normalizeParagraph(node);
    case "heading":
      return [normalizeHeading(node)];
    case "bulletList":
    case "orderedList":
    case "taskList":
      return [normalizeList(node)];
    case "blockquote":
      return [
        {
          ...node,
          content: normalizeBlockContainerContent(node.content),
        },
      ];
    case "codeBlock":
      return [normalizeCodeBlock(node)];
    case "image":
    case "horizontalRule":
      return [node];
    case "text":
    case "hardBreak":
      return [createParagraph([sanitizeMarks(node)])];
    default:
      if (node.type?.startsWith("mention-")) {
        return [createParagraph([node])];
      }
      return [];
  }
}

function normalizeParagraph(node: JSONContent): JSONContent[] {
  const blocks: JSONContent[] = [];
  let inlineContent: JSONContent[] = [];

  const flushParagraph = () => {
    if (inlineContent.length > 0) {
      blocks.push(createParagraph(inlineContent));
      inlineContent = [];
    }
  };

  for (const child of node.content ?? []) {
    if (isInlineNode(child)) {
      inlineContent.push(child.type === "text" ? sanitizeMarks(child) : child);
      continue;
    }

    switch (child.type) {
      case "image":
        flushParagraph();
        blocks.push(child);
        break;
      case "heading":
        flushParagraph();
        blocks.push(createParagraph(normalizeInlineContent(child.content)));
        break;
      case "paragraph":
        flushParagraph();
        blocks.push(...normalizeParagraph(child));
        break;
      case "bulletList":
      case "orderedList":
      case "taskList":
        flushParagraph();
        blocks.push(normalizeList(child));
        break;
      case "blockquote":
        flushParagraph();
        blocks.push({
          ...child,
          content: normalizeBlockContainerContent(child.content),
        });
        break;
      case "codeBlock":
        flushParagraph();
        blocks.push(normalizeCodeBlock(child));
        break;
      case "horizontalRule":
        flushParagraph();
        blocks.push(child);
        break;
      default:
        if (child.type?.startsWith("mention-")) {
          inlineContent.push(child);
        }
        break;
    }
  }

  flushParagraph();

  return blocks.length > 0 ? blocks : [createParagraph()];
}

function normalizeHeading(node: JSONContent): JSONContent {
  const content = normalizeInlineContent(node.content);
  return content.length > 0 ? { ...node, content } : { ...node };
}

function normalizeList(node: JSONContent): JSONContent {
  const itemType = node.type === "taskList" ? "taskItem" : "listItem";
  const items = (node.content ?? []).map((child) =>
    normalizeListItem(child, itemType),
  );

  return {
    ...node,
    content: items.length > 0 ? items : [createListItem(itemType)],
  };
}

function normalizeListItem(
  node: JSONContent,
  itemType: "listItem" | "taskItem",
): JSONContent {
  const blocks =
    node.type === itemType
      ? (node.content ?? []).flatMap(normalizeListItemChild)
      : normalizeListItemChild(node);

  if (blocks.length === 0 || blocks[0]?.type !== "paragraph") {
    blocks.unshift(createParagraph());
  }

  return createListItem(itemType, blocks, node.attrs);
}

function normalizeListItemChild(node: JSONContent): JSONContent[] {
  switch (node.type) {
    case "paragraph":
      return normalizeParagraph(node);
    case "heading":
      return [createParagraph(normalizeInlineContent(node.content))];
    case "bulletList":
    case "orderedList":
    case "taskList":
      return [normalizeList(node)];
    case "blockquote":
      return [
        {
          ...node,
          content: normalizeBlockContainerContent(node.content),
        },
      ];
    case "codeBlock":
      return [normalizeCodeBlock(node)];
    case "image":
    case "horizontalRule":
      return [node];
    case "text":
    case "hardBreak":
      return [createParagraph([sanitizeMarks(node)])];
    default:
      if (node.type?.startsWith("mention-")) {
        return [createParagraph([node])];
      }
      return [];
  }
}

function normalizeCodeBlock(node: JSONContent): JSONContent {
  const content = (node.content ?? []).flatMap((child) =>
    child.type === "text" ? [{ ...child, marks: undefined }] : [],
  );

  return content.length > 0 ? { ...node, content } : { ...node, content: [] };
}

function normalizeInlineContent(
  content: JSONContent[] | undefined,
): JSONContent[] {
  const normalized: JSONContent[] = [];

  for (const child of content ?? []) {
    if (isInlineNode(child)) {
      normalized.push(child.type === "text" ? sanitizeMarks(child) : child);
      continue;
    }

    if (child.type === "image") {
      const alt = child.attrs?.alt;
      if (typeof alt === "string" && alt.length > 0) {
        normalized.push({ type: "text", text: alt });
      }
    }
  }

  return normalized;
}

function isInlineNode(node: JSONContent): boolean {
  return (
    node.type === "text" ||
    node.type === "hardBreak" ||
    node.type?.startsWith("mention-") === true
  );
}

function createParagraph(content: JSONContent[] = []): JSONContent {
  return content.length > 0
    ? { type: "paragraph", content }
    : { type: "paragraph" };
}

function createListItem(
  itemType: "listItem" | "taskItem",
  content: JSONContent[] = [createParagraph()],
  attrs?: JSONContent["attrs"],
): JSONContent {
  if (itemType === "taskItem") {
    return {
      type: "taskItem",
      attrs: { checked: attrs?.checked === true },
      content,
    };
  }

  return {
    type: "listItem",
    content,
  };
}

function createFallbackDoc(text?: string): JSONContent {
  if (typeof text === "string" && text.length > 0) {
    return {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text }],
        },
      ],
    };
  }

  return EMPTY_TIPTAP_DOC;
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
