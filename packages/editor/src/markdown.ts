import { Node as PMNode } from "prosemirror-model";

import { getParser } from "./markdown/parser";
import { markdownSchema } from "./markdown/schema";
import { getSerializer } from "./markdown/serializer";

export { markdownSchema } from "./markdown/schema";

// ---------------------------------------------------------------------------

function liftBlockImages(doc: JSONContent): JSONContent {
  if (!doc.content) return doc;

  const out: JSONContent[] = [];
  for (const node of doc.content) {
    if (
      node.type === "paragraph" &&
      node.content &&
      node.content.length === 1 &&
      node.content[0].type === "image"
    ) {
      out.push(node.content[0]);
    } else {
      out.push(node);
    }
  }
  return { ...doc, content: out };
}

function wrapBlockImages(doc: JSONContent): JSONContent {
  if (!doc.content) return doc;

  const out: JSONContent[] = [];
  for (const node of doc.content) {
    if (node.type === "image") {
      out.push({ type: "paragraph", content: [node] });
    } else {
      out.push(node);
    }
  }
  return { ...doc, content: out };
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

export interface JSONContent {
  type?: string;
  attrs?: Record<string, any>;
  content?: JSONContent[];
  marks?: { type: string; attrs?: Record<string, any> }[];
  text?: string;
}

export const EMPTY_DOC: JSONContent = {
  type: "doc",
  content: [{ type: "paragraph" }],
};

export function isValidContent(content: unknown): content is JSONContent {
  if (!content || typeof content !== "object") {
    return false;
  }
  const obj = content as Record<string, unknown>;
  return obj.type === "doc" && Array.isArray(obj.content);
}

export function parseJsonContent(raw: string | undefined | null): JSONContent {
  if (typeof raw !== "string" || !raw.trim()) {
    return EMPTY_DOC;
  }
  try {
    const parsed = JSON.parse(raw);
    return isValidContent(parsed) ? parsed : EMPTY_DOC;
  } catch {
    return EMPTY_DOC;
  }
}

export function md2json(markdown: string): JSONContent {
  try {
    const doc = getParser().parse(markdown);
    const json = doc.toJSON() as JSONContent;
    if (!json.content || json.content.length === 0) {
      return { type: "doc", content: [{ type: "paragraph" }] };
    }
    return liftBlockImages(json);
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

export function json2md(jsonContent: JSONContent): string {
  try {
    const wrapped = wrapBlockImages(jsonContent);
    const doc = PMNode.fromJSON(markdownSchema, wrapped);
    return getSerializer().serialize(doc);
  } catch (error) {
    console.error(error);
    return "";
  }
}
