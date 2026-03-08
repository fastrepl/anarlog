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

export function safeParseJSON(value: unknown): unknown {
  if (typeof value !== "string") {
    return value;
  }

  try {
    return JSON.parse(value);
  } catch {
    return value;
  }
}

export function normalizeQuery(query: string): string {
  return query.trim().replace(SPACE_REGEX, " ");
}

export interface ParsedQuery {
  term: string;
  exact: boolean;
}

export function parseQuery(query: string): ParsedQuery {
  const normalized = normalizeQuery(query);
  const quoteMatch = normalized.match(/^"(.+)"$/);
  if (quoteMatch) {
    return { term: quoteMatch[1], exact: true };
  }
  return { term: normalized, exact: false };
}

export function toTrimmedString(value: unknown): string {
  if (typeof value === "string") {
    return value.trim();
  }

  return "";
}

export function toNumber(value: unknown): number {
  if (typeof value === "number") {
    return value;
  }
  if (typeof value === "string") {
    const parsed = Number(value);
    return isNaN(parsed) ? 0 : parsed;
  }
  return 0;
}

export function toEpochMs(value: unknown): number {
  if (typeof value === "number") {
    return value;
  }
  if (typeof value === "string") {
    const parsed = Date.parse(value);
    if (!isNaN(parsed)) {
      return parsed;
    }
    const numParsed = Number(value);
    return isNaN(numParsed) ? 0 : numParsed;
  }
  return 0;
}

export function toString(value: unknown): string {
  if (typeof value === "string" && value.length > 0) {
    return value;
  }
  return "";
}

export function toBoolean(value: unknown): boolean {
  if (typeof value === "boolean") {
    return value;
  }
  return false;
}

export function mergeContent(parts: unknown[]): string {
  return parts.map(toTrimmedString).filter(Boolean).join(" ");
}

export function flattenTranscript(transcript: unknown): string {
  if (transcript == null) {
    return "";
  }

  const parsed = safeParseJSON(transcript);

  if (typeof parsed === "string") {
    return parsed;
  }

  if (Array.isArray(parsed)) {
    return mergeContent(
      parsed.map((segment) => {
        if (!segment) {
          return "";
        }

        if (typeof segment === "string") {
          return segment;
        }

        if (typeof segment === "object") {
          const record = segment as Record<string, unknown>;
          const preferred = record.text ?? record.content;
          if (typeof preferred === "string") {
            return preferred;
          }

          return flattenTranscript(Object.values(record));
        }

        return "";
      }),
    );
  }

  if (typeof parsed === "object" && parsed !== null) {
    return mergeContent(
      Object.values(parsed).map((value) => flattenTranscript(value)),
    );
  }

  return "";
}

export function collectCells(
  persistedStore: any,
  table: string,
  rowId: string,
  fields: string[],
): Record<string, unknown> {
  return fields.reduce<Record<string, unknown>>((acc, field) => {
    acc[field] = persistedStore.getCell(table, rowId, field);
    return acc;
  }, {});
}

export { collectEnhancedNotesContent } from "~/store/tinybase/store/utils";
