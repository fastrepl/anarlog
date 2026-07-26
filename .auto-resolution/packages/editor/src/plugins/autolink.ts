import { Plugin, PluginKey, type Transaction } from "prosemirror-state";

import { getChangedTextblockRanges } from "./changed-ranges";
import { isValidUrl, normalizeUrlHref } from "./url";

const URL_CANDIDATE_REGEX =
  /(^|[^\p{L}\p{N}_@.-])((?:https?:\/\/|www\.)[^\s<>"'`]+|(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}(?::\d{1,5})?(?:[/?#][^\s<>"'`]*)?)/giu;
const TRAILING_PUNCTUATION = new Set([".", ",", "!", "?", ";", ":"]);
const CLOSING_TO_OPENING: Record<string, string> = {
  ")": "(",
  "]": "[",
  "}": "{",
};

function countChar(text: string, char: string) {
  return [...text].filter((c) => c === char).length;
}

function trimTrailingPunctuation(text: string) {
  let end = text.length;

  while (end > 0) {
    const char = text[end - 1];
    if (TRAILING_PUNCTUATION.has(char)) {
      end -= 1;
      continue;
    }

    const opener = CLOSING_TO_OPENING[char];
    if (
      opener &&
      countChar(text.slice(0, end), char) >
        countChar(text.slice(0, end), opener)
    ) {
      end -= 1;
      continue;
    }

    break;
  }

  return text.slice(0, end);
}

export function findAutolinkMatches(text: string) {
  const matches: Array<{ start: number; end: number; href: string }> = [];
  let match: RegExpExecArray | null;

  URL_CANDIDATE_REGEX.lastIndex = 0;

  while ((match = URL_CANDIDATE_REGEX.exec(text)) !== null) {
    const boundary = match[1] ?? "";
    const candidate = trimTrailingPunctuation(match[2] ?? "");
    if (!candidate || !isValidUrl(candidate)) {
      continue;
    }

    const start = match.index + boundary.length;
    matches.push({
      start,
      end: start + candidate.length,
      href: normalizeUrlHref(candidate),
    });
  }

  return matches;
}

export function autolinkPlugin() {
  return new Plugin({
    key: new PluginKey("autolink"),
    appendTransaction(transactions, _oldState, newState) {
      if (!transactions.some((tr) => tr.docChanged)) return null;

      const linkType = newState.schema.marks.link;
      if (!linkType) return null;

      let tr: Transaction | null = null;

      const changedTextblockRanges = getChangedTextblockRanges(
        newState.doc,
        transactions,
      );

      for (const range of changedTextblockRanges) {
        newState.doc.nodesBetween(range.from, range.to, (node, pos, parent) => {
          if (!node.isText || !node.text || parent?.type.spec.code) {
            return;
          }

          for (const match of findAutolinkMatches(node.text)) {
            const from = pos + match.start;
            const to = pos + match.end;
            if (newState.doc.rangeHasMark(from, to, linkType)) {
              continue;
            }

            if (!tr) tr = newState.tr;
            tr.addMark(
              from,
              to,
              linkType.create({ href: match.href, target: null }),
            );
          }
        });
      }

      return tr;
    },
  });
}
