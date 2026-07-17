import type { Node as PMNode } from "prosemirror-model";
import type { Transaction } from "prosemirror-state";

export type ChangedRange = {
  from: number;
  to: number;
};

export function getChangedRanges(transactions: readonly Transaction[]) {
  const ranges: ChangedRange[] = [];

  for (const transaction of transactions) {
    if (!transaction.docChanged) continue;
    transaction.mapping.maps.forEach((stepMap) => {
      stepMap.forEach((_oldStart, _oldEnd, newStart, newEnd) => {
        ranges.push({ from: newStart, to: newEnd });
      });
    });
  }

  return ranges;
}

export function hasChangedRange(
  ranges: readonly ChangedRange[],
  from: number,
  to: number,
) {
  return ranges.some((range) =>
    range.from === range.to
      ? from <= range.from && range.from <= to
      : range.from < to && from < range.to,
  );
}

export function getChangedTextblockRanges(
  doc: PMNode,
  transactions: readonly Transaction[],
) {
  return mergeRanges(
    getChangedRanges(transactions).flatMap((range) =>
      getTextblockRangesForRange(doc, range),
    ),
  );
}

export function hasChangedNodeOfType(
  doc: PMNode,
  transactions: readonly Transaction[],
  nodeTypeName: string,
) {
  return getChangedRanges(transactions).some((range) => {
    if (
      hasAncestorNodeOfType(doc, range.from, nodeTypeName) ||
      hasAncestorNodeOfType(doc, range.to, nodeTypeName)
    ) {
      return true;
    }

    let found = false;
    const from = clampPosition(range.from, doc.content.size);
    const to = clampPosition(Math.max(range.to, range.from), doc.content.size);
    doc.nodesBetween(from, to, (node) => {
      if (node.type.name === nodeTypeName) {
        found = true;
        return false;
      }

      return !found;
    });
    return found;
  });
}

function hasAncestorNodeOfType(doc: PMNode, pos: number, nodeTypeName: string) {
  const resolved = doc.resolve(clampPosition(pos, doc.content.size));
  for (let depth = resolved.depth; depth > 0; depth--) {
    if (resolved.node(depth).type.name === nodeTypeName) {
      return true;
    }
  }

  return false;
}

function getTextblockRangesForRange(doc: PMNode, range: ChangedRange) {
  const ranges: ChangedRange[] = [];
  const docSize = doc.content.size;
  const from = clampPosition(range.from, docSize);
  const to = clampPosition(Math.max(range.to, range.from), docSize);

  addParentTextblockRange(doc, from, ranges);
  addParentTextblockRange(doc, to, ranges);

  if (from < to) {
    doc.nodesBetween(from, to, (node, pos) => {
      if (!node.isTextblock) {
        return true;
      }

      ranges.push({
        from: pos + 1,
        to: pos + node.nodeSize - 1,
      });
      return false;
    });
  }

  return ranges;
}

function addParentTextblockRange(
  doc: PMNode,
  pos: number,
  ranges: ChangedRange[],
) {
  const resolved = doc.resolve(pos);
  for (let depth = resolved.depth; depth > 0; depth--) {
    const node = resolved.node(depth);
    if (!node.isTextblock) {
      continue;
    }

    ranges.push({ from: resolved.start(depth), to: resolved.end(depth) });
    return;
  }
}

function mergeRanges(ranges: ChangedRange[]) {
  const sorted = ranges
    .filter((range) => range.to >= range.from)
    .sort((a, b) => a.from - b.from || a.to - b.to);
  const merged: ChangedRange[] = [];

  for (const range of sorted) {
    const last = merged[merged.length - 1];
    if (!last || range.from > last.to) {
      merged.push({ ...range });
      continue;
    }

    last.to = Math.max(last.to, range.to);
  }

  return merged;
}

function clampPosition(pos: number, docSize: number) {
  return Math.max(0, Math.min(pos, docSize));
}
