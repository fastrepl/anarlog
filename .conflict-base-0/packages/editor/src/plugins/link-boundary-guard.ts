import type { Attrs } from "prosemirror-model";
import { Plugin, PluginKey, type Transaction } from "prosemirror-state";

import {
  getChangedRanges,
  getChangedTextblockRanges,
  hasChangedRange,
} from "./changed-ranges";
import {
  isLinkTextForHref,
  isValidUrl,
  looksLikeUrlText,
  normalizeUrlHref,
} from "./url";

const STANDALONE_TRAILING_PUNCTUATION_REGEX = /^[.,!?;:)\]}]+$/;

export function linkBoundaryGuardPlugin() {
  return new Plugin({
    key: new PluginKey("linkBoundaryGuard"),
    appendTransaction(transactions, _oldState, newState) {
      if (!transactions.some((tr) => tr.docChanged)) return null;
      const linkType = newState.schema.marks.link;
      if (!linkType) return null;

      let tr: Transaction | null = null;
      let prevLink: {
        startPos: number;
        endPos: number;
        attrs: Attrs;
        href: string;
      } | null = null;
      const changedRanges = getChangedRanges(transactions);
      const changedTextblockRanges = getChangedTextblockRanges(
        newState.doc,
        transactions,
      );

      for (const range of changedTextblockRanges) {
        prevLink = null;
        newState.doc.nodesBetween(range.from, range.to, (node, pos) => {
          if (!node.isText || !node.text) {
            prevLink = null;
            return;
          }

          const linkMark = node.marks.find((m) => m.type === linkType);

          if (linkMark) {
            const textLooksLikeUrl = looksLikeUrlText(node.text);
            const href = linkMark.attrs.href;
            const nodeTextChanged = hasChangedRange(
              changedRanges,
              pos,
              pos + node.text.length,
            );

            if (typeof href !== "string") {
              prevLink = null;
              return;
            }

            if (textLooksLikeUrl && !isValidUrl(node.text) && nodeTextChanged) {
              if (!tr) tr = newState.tr;
              tr.removeMark(pos, pos + node.text.length, linkType);
              prevLink = null;
            } else if (isLinkTextForHref(node.text, href)) {
              prevLink = {
                startPos: pos,
                endPos: pos + node.text.length,
                attrs: linkMark.attrs,
                href,
              };
            } else if (textLooksLikeUrl && nodeTextChanged) {
              const nextHref = normalizeUrlHref(node.text);
              if (!tr) tr = newState.tr;
              tr.removeMark(pos, pos + node.text.length, linkType);
              tr.addMark(
                pos,
                pos + node.text.length,
                linkType.create({ ...linkMark.attrs, href: nextHref }),
              );
              prevLink = {
                startPos: pos,
                endPos: pos + node.text.length,
                attrs: { ...linkMark.attrs, href: nextHref },
                href: nextHref,
              };
            } else if (textLooksLikeUrl) {
              prevLink = {
                startPos: pos,
                endPos: pos + node.text.length,
                attrs: linkMark.attrs,
                href,
              };
            } else {
              prevLink = null;
            }
          } else if (prevLink && pos === prevLink.endPos && node.text) {
            if (!/^\s/.test(node.text[0])) {
              const wsIdx = node.text.search(/\s/);
              const extendLen = wsIdx >= 0 ? wsIdx : node.text.length;
              const extension = node.text.slice(0, extendLen);
              const newHref = prevLink.href + extension;
              if (
                !STANDALONE_TRAILING_PUNCTUATION_REGEX.test(extension) &&
                isValidUrl(newHref)
              ) {
                if (!tr) tr = newState.tr;
                tr.removeMark(prevLink.startPos, prevLink.endPos, linkType);
                tr.addMark(
                  prevLink.startPos,
                  pos + extendLen,
                  linkType.create({
                    ...prevLink.attrs,
                    href: newHref,
                  }),
                );
              }
            }
            prevLink = null;
          } else {
            prevLink = null;
          }
        });
      }

      return tr;
    },
  });
}
