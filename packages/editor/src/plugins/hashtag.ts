import { type Node as PMNode } from "prosemirror-model";
import { Plugin, PluginKey } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";

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
    props: {
      decorations(state) {
        const { doc } = state;
        const decorations: Decoration[] = [];

        doc.descendants((node: PMNode, pos: number) => {
          if (!node.isText || !node.text) return;
          for (const match of findHashtags(node.text)) {
            decorations.push(
              Decoration.inline(pos + match.start, pos + match.end, {
                class: "hashtag",
              }),
            );
          }
        });

        return DecorationSet.create(doc, decorations);
      },
    },
  });
}
