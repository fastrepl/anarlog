import { type Node as PMNode } from "prosemirror-model";
import { Plugin, PluginKey } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";

import { getChangedTextblockRanges, type ChangedRange } from "./changed-ranges";

const HASHTAG_REGEX = /#([\p{L}\p{N}_\p{Emoji}\p{Emoji_Component}]+)/gu;
const LEADING_PUNCTUATION_REGEX = /^[([{<"'`]+/u;
const HTTP_PREFIXES = ["http://", "https://", "www."];

const normalizeUrlToken = (token: string): string => {
  const normalized = token.replace(LEADING_PUNCTUATION_REGEX, "");

  if (normalized.toLowerCase().startsWith("www.")) {
    return `https://${normalized}`;
  }

  return normalized;
};

const isUrlFragmentHashtag = (text: string, hashtagStart: number): boolean => {
  const beforeHashtag = text.slice(0, hashtagStart);
  const tokenStart = beforeHashtag.search(/\S+$/u);

  if (tokenStart < 0) {
    return false;
  }

  const token = beforeHashtag.slice(tokenStart);
  const normalizedToken = token
    .replace(LEADING_PUNCTUATION_REGEX, "")
    .toLowerCase();

  if (!HTTP_PREFIXES.some((prefix) => normalizedToken.startsWith(prefix))) {
    return false;
  }

  try {
    const parsed = new URL(normalizeUrlToken(token));
    return Boolean(parsed.hostname && parsed.hostname.includes("."));
  } catch {
    return false;
  }
};

export const findHashtags = (
  text: string,
): Array<{ tag: string; start: number; end: number }> => {
  const matches: Array<{ tag: string; start: number; end: number }> = [];
  let match;

  HASHTAG_REGEX.lastIndex = 0;

  while ((match = HASHTAG_REGEX.exec(text)) !== null) {
    const start = match.index;

    if (isUrlFragmentHashtag(text, start)) {
      continue;
    }

    const tag = match[1].trim();

    if (!tag) {
      continue;
    }

    matches.push({
      tag,
      start,
      end: start + match[0].length,
    });
  }

  return matches;
};

export const hashtagPluginKey = new PluginKey("hashtagDecoration");

export function hashtagPlugin() {
  return new Plugin({
    key: hashtagPluginKey,
    state: {
      init(_config, state) {
        return buildHashtagDecorationSet(state.doc);
      },
      apply(tr, decorationSet) {
        let next = decorationSet.map(tr.mapping, tr.doc);
        if (!tr.docChanged) {
          return next;
        }

        const changedTextblockRanges = getChangedTextblockRanges(tr.doc, [tr]);
        for (const range of changedTextblockRanges) {
          next = next.remove(next.find(range.from, range.to));
        }

        return next.add(
          tr.doc,
          buildHashtagDecorations(tr.doc, changedTextblockRanges),
        );
      },
    },
    props: {
      decorations(state) {
        return hashtagPluginKey.getState(state);
      },
    },
  });
}

function buildHashtagDecorationSet(doc: PMNode) {
  return DecorationSet.create(doc, buildHashtagDecorations(doc));
}

function buildHashtagDecorations(doc: PMNode, ranges?: ChangedRange[]) {
  const decorations: Decoration[] = [];
  const visitRange = (from: number, to: number) => {
    doc.nodesBetween(from, to, (node: PMNode, pos: number) => {
      if (!node.isText || !node.text) return;
      for (const match of findHashtags(node.text)) {
        decorations.push(
          Decoration.inline(pos + match.start, pos + match.end, {
            class: "hashtag",
          }),
        );
      }
    });
  };

  if (ranges) {
    for (const range of ranges) {
      visitRange(range.from, range.to);
    }
  } else {
    visitRange(0, doc.content.size);
  }

  return decorations;
}
