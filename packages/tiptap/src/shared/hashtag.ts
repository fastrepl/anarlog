import { Extension } from "@tiptap/core";
import { Node as ProseMirrorNode } from "@tiptap/pm/model";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";

export const HASHTAG_REGEX = /#([\p{L}\p{N}_\p{Emoji}\p{Emoji_Component}]+)/gu;
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

export function extractHashtags(content: string): string[] {
  const tempDiv = document.createElement("div");
  tempDiv.innerHTML = content;

  const hashtags: string[] = [];
  const textNodes: Node[] = [];

  function getTextNodes(node: Node) {
    if (node.nodeType === Node.TEXT_NODE) {
      textNodes.push(node);
    } else {
      for (let i = 0; i < node.childNodes.length; i++) {
        getTextNodes(node.childNodes[i]);
      }
    }
  }

  getTextNodes(tempDiv);

  textNodes.forEach((node) => {
    const text = node.textContent || "";
    hashtags.push(...findHashtags(text).map((match) => match.tag));
  });

  const uniqueTags = [...new Set(hashtags)];
  return uniqueTags;
}

export const Hashtag = Extension.create({
  name: "hashtag",

  addProseMirrorPlugins() {
    const decorationPlugin = new Plugin({
      key: new PluginKey("hashtagDecoration"),
      props: {
        decorations(state) {
          const { doc } = state;
          const decorations: Decoration[] = [];

          doc.descendants((node: ProseMirrorNode, pos: number) => {
            if (!node.isText) {
              return;
            }

            const text = node.text as string;
            const matches = findHashtags(text);

            for (const match of matches) {
              const start = pos + match.start;
              const end = pos + match.end;

              decorations.push(
                Decoration.inline(start, end, {
                  class: "hashtag",
                }),
              );
            }
          });

          return DecorationSet.create(doc, decorations);
        },
      },
    });

    return [decorationPlugin];
  },
});
