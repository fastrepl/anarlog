import { type Node as PMNode } from "prosemirror-model";

export const ANCHOR_BLOCK_SEPARATOR = "\n";
export const ANCHOR_LEAF_TEXT = "￼";

type Segment = {
  kind: "text" | "leaf" | "separator";
  textStart: number;
  textEnd: number;
  posStart: number;
  posEnd: number;
};

export type DocTextIndex = {
  text: string;
  /** Text offset of a match start → ProseMirror position (skips separators forward). */
  posAt(index: number): number;
  /** Exclusive text offset of a match end → ProseMirror position (skips separators backward). */
  endPosAt(index: number): number;
  /** ProseMirror position → text offset. */
  indexAt(pos: number): number;
  /** Whether the character at this text offset is a block separator (as opposed to a real newline inside a code block). */
  isSeparatorAt(index: number): boolean;
};

/**
 * Builds a text projection of the document equal to
 * `doc.textBetween(0, doc.content.size, ANCHOR_BLOCK_SEPARATOR, ANCHOR_LEAF_TEXT)`
 * together with an offset↔position mapping. Capture and resolution share this
 * one traversal so surfaces can never disagree on separator semantics.
 */
export function buildDocTextIndex(doc: PMNode): DocTextIndex {
  const segments: Segment[] = [];
  let text = "";
  let first = true;

  doc.nodesBetween(0, doc.content.size, (node, pos) => {
    const leafText = node.isText ? "" : node.isLeaf ? ANCHOR_LEAF_TEXT : "";
    const nodeText = node.isText ? (node.text ?? "") : leafText;
    if (
      node.isBlock &&
      ((node.isLeaf && nodeText !== "") || node.isTextblock)
    ) {
      if (first) {
        first = false;
      } else {
        segments.push({
          kind: "separator",
          textStart: text.length,
          textEnd: text.length + ANCHOR_BLOCK_SEPARATOR.length,
          posStart: pos,
          posEnd: pos,
        });
        text += ANCHOR_BLOCK_SEPARATOR;
      }
    }
    if (node.isText) {
      segments.push({
        kind: "text",
        textStart: text.length,
        textEnd: text.length + nodeText.length,
        posStart: pos,
        posEnd: pos + nodeText.length,
      });
      text += nodeText;
    } else if (node.isLeaf && nodeText !== "") {
      segments.push({
        kind: "leaf",
        textStart: text.length,
        textEnd: text.length + nodeText.length,
        posStart: pos,
        posEnd: pos + node.nodeSize,
      });
      text += nodeText;
    }
    return undefined;
  });

  const contentSegments = segments.filter(
    (segment) => segment.kind !== "separator",
  );
  const separatorOffsets = new Set<number>();
  for (const segment of segments) {
    if (segment.kind !== "separator") continue;
    for (
      let offset = segment.textStart;
      offset < segment.textEnd;
      offset += 1
    ) {
      separatorOffsets.add(offset);
    }
  }

  const posAt = (index: number): number => {
    for (const segment of contentSegments) {
      if (index >= segment.textEnd) continue;
      if (index < segment.textStart) return segment.posStart;
      return segment.kind === "text"
        ? segment.posStart + (index - segment.textStart)
        : segment.posStart;
    }
    const last = contentSegments[contentSegments.length - 1];
    return last ? last.posEnd : 0;
  };

  const endPosAt = (index: number): number => {
    for (let i = contentSegments.length - 1; i >= 0; i -= 1) {
      const segment = contentSegments[i];
      if (index <= segment.textStart) continue;
      if (index > segment.textEnd) return segment.posEnd;
      return segment.kind === "text"
        ? segment.posStart + (index - segment.textStart)
        : segment.posEnd;
    }
    const firstSegment = contentSegments[0];
    return firstSegment ? firstSegment.posStart : 0;
  };

  const indexAt = (pos: number): number => {
    for (const segment of contentSegments) {
      if (pos < segment.posStart) return segment.textStart;
      if (pos <= segment.posEnd) {
        return segment.kind === "text"
          ? segment.textStart + (pos - segment.posStart)
          : pos === segment.posStart
            ? segment.textStart
            : segment.textEnd;
      }
    }
    return text.length;
  };

  return {
    text,
    posAt,
    endPosAt,
    indexAt,
    isSeparatorAt: (index: number) => separatorOffsets.has(index),
  };
}
