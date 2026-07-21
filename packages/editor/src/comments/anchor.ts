import { type Node as PMNode } from "prosemirror-model";

import {
  ANCHOR_LEAF_TEXT,
  buildDocTextIndex,
  type DocTextIndex,
} from "./anchor-text";

export const ANCHOR_CONTEXT_LENGTH = 64;
export const MAX_ANCHOR_QUOTE_LENGTH = 1024;

export type CommentAnchor = {
  quoteExact: string;
  quotePrefix: string;
  quoteSuffix: string;
  fromHint: number | null;
  toHint: number | null;
  snapshotRevision: number;
};

export type ResolvedAnchorRange = { from: number; to: number };

/**
 * Captures a quote anchor for the selected range. Returns null when the
 * selection has no anchorable text (empty, or only leaf placeholders) or is
 * longer than MAX_ANCHOR_QUOTE_LENGTH — callers hide the comment affordance.
 */
export function captureCommentAnchor(
  doc: PMNode,
  from: number,
  to: number,
  snapshotRevision: number,
): CommentAnchor | null {
  if (to <= from) return null;
  const index = buildDocTextIndex(doc);
  let fromIndex = index.indexAt(from);
  let toIndex = index.indexAt(to);
  // Trim boundary block separators: a quote stored with one would stop
  // matching when the neighboring block changes, even though every visibly
  // selected character still exists. Real newlines inside code blocks are
  // content and must be kept, so this checks positions, not characters.
  while (fromIndex < toIndex && index.isSeparatorAt(fromIndex)) fromIndex += 1;
  while (toIndex > fromIndex && index.isSeparatorAt(toIndex - 1)) toIndex -= 1;
  const quoteExact = index.text.slice(fromIndex, toIndex);
  if (
    quoteExact.length === 0 ||
    quoteExact.length > MAX_ANCHOR_QUOTE_LENGTH ||
    !hasAnchorableContent(index, fromIndex, toIndex)
  ) {
    return null;
  }
  return {
    quoteExact,
    quotePrefix: index.text.slice(
      Math.max(0, fromIndex - ANCHOR_CONTEXT_LENGTH),
      fromIndex,
    ),
    quoteSuffix: index.text.slice(toIndex, toIndex + ANCHOR_CONTEXT_LENGTH),
    fromHint: index.posAt(fromIndex),
    toHint: index.endPosAt(toIndex),
    snapshotRevision,
  };
}

/**
 * Resolves an anchor against a document. The hint fast path is only trusted
 * when the revision matches AND the hinted slice still equals the quote;
 * otherwise the quote is searched. Several occurrences are disambiguated by
 * prefix/suffix overlap; a tie resolves to null (unanchored) — never a guess.
 */
export function resolveCommentAnchor(
  doc: PMNode,
  anchor: CommentAnchor,
  currentRevision: number,
  index: DocTextIndex = buildDocTextIndex(doc),
): ResolvedAnchorRange | null {
  const { quoteExact } = anchor;
  if (quoteExact.length === 0) return null;

  if (
    anchor.snapshotRevision === currentRevision &&
    anchor.fromHint !== null &&
    anchor.toHint !== null &&
    anchor.fromHint >= 0 &&
    anchor.toHint <= doc.content.size &&
    anchor.fromHint < anchor.toHint
  ) {
    const hintedSlice = index.text.slice(
      index.indexAt(anchor.fromHint),
      index.indexAt(anchor.toHint),
    );
    if (hintedSlice === quoteExact) {
      return { from: anchor.fromHint, to: anchor.toHint };
    }
  }

  const occurrences: number[] = [];
  let cursor = index.text.indexOf(quoteExact);
  while (cursor !== -1) {
    occurrences.push(cursor);
    cursor = index.text.indexOf(quoteExact, cursor + 1);
  }
  if (occurrences.length === 0) return null;

  let winner: number;
  if (occurrences.length === 1) {
    winner = occurrences[0];
  } else {
    let bestScore = -1;
    let bestStart = -1;
    let tie = false;
    for (const start of occurrences) {
      const score = contextScore(index.text, start, quoteExact.length, anchor);
      if (score > bestScore) {
        bestScore = score;
        bestStart = start;
        tie = false;
      } else if (score === bestScore) {
        tie = true;
      }
    }
    if (tie) return null;
    winner = bestStart;
  }

  const from = index.posAt(winner);
  const to = index.endPosAt(winner + quoteExact.length);
  return from < to ? { from, to } : null;
}

/** Batch resolution that builds the text index once. */
export function resolveCommentAnchors<
  T extends { commentId: string; anchor: CommentAnchor | null },
>(
  doc: PMNode,
  comments: readonly T[],
  currentRevision: number,
): Array<{ commentId: string; range: ResolvedAnchorRange | null }> {
  const index = buildDocTextIndex(doc);
  return comments.map((comment) => ({
    commentId: comment.commentId,
    range: comment.anchor
      ? resolveCommentAnchor(doc, comment.anchor, currentRevision, index)
      : null,
  }));
}

function contextScore(
  text: string,
  start: number,
  quoteLength: number,
  anchor: CommentAnchor,
): number {
  const before = text.slice(Math.max(0, start - ANCHOR_CONTEXT_LENGTH), start);
  const after = text.slice(
    start + quoteLength,
    start + quoteLength + ANCHOR_CONTEXT_LENGTH,
  );
  return (
    commonSuffixLength(anchor.quotePrefix, before) +
    commonPrefixLength(anchor.quoteSuffix, after)
  );
}

function commonPrefixLength(left: string, right: string): number {
  const max = Math.min(left.length, right.length);
  let length = 0;
  while (length < max && left[length] === right[length]) length += 1;
  return length;
}

function commonSuffixLength(left: string, right: string): number {
  const max = Math.min(left.length, right.length);
  let length = 0;
  while (
    length < max &&
    left[left.length - 1 - length] === right[right.length - 1 - length]
  ) {
    length += 1;
  }
  return length;
}

function hasAnchorableContent(
  index: DocTextIndex,
  fromIndex: number,
  toIndex: number,
): boolean {
  for (let offset = fromIndex; offset < toIndex; offset += 1) {
    const character = index.text[offset];
    if (character === ANCHOR_LEAF_TEXT) continue;
    if (character === "\n" && index.isSeparatorAt(offset)) continue;
    return true;
  }
  return false;
}
