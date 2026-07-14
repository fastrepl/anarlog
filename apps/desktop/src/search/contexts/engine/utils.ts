const SPACE_REGEX = /\s+/g;

interface TiptapNode {
  type: string;
  content?: TiptapNode[];
  text?: string;
}

function isValidTiptapContent(content: unknown): content is TiptapNode {
  if (!content || typeof content !== "object") {
    return false;
  }
  const obj = content as Record<string, unknown>;
  return obj.type === "doc" && Array.isArray(obj.content);
}

function extractTextFromTiptapNode(node: TiptapNode): string {
  if (node.text) {
    return node.text;
  }
  if (node.content && Array.isArray(node.content)) {
    return node.content.map(extractTextFromTiptapNode).join(" ");
  }
  return "";
}

export function extractPlainText(value: unknown): string {
  if (typeof value !== "string" || !value.trim()) {
    return "";
  }

  const trimmed = value.trim();
  if (!trimmed.startsWith("{")) {
    return trimmed;
  }

  try {
    const parsed = JSON.parse(trimmed);
    if (isValidTiptapContent(parsed)) {
      const text = extractTextFromTiptapNode(parsed).trim();
      return text.replace(SPACE_REGEX, " ");
    }
    return trimmed;
  } catch {
    return trimmed;
  }
}

export function normalizeQuery(query: string): string {
  return query.trim().replace(SPACE_REGEX, " ");
}
