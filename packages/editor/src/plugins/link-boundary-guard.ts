import { type Mark } from "prosemirror-model";
import { Plugin, PluginKey, type Transaction } from "prosemirror-state";
import tldList from "tlds";

const VALID_TLDS = new Set(tldList.map((t: string) => t.toLowerCase()));

function isValidUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return false;
    }
    const parts = parsed.hostname.split(".");
    if (parts.length < 2) return false;
    return VALID_TLDS.has(parts[parts.length - 1].toLowerCase());
  } catch {
    return false;
  }
}

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
        mark: Mark;
      } | null = null;

      newState.doc.descendants((node, pos) => {
        if (!node.isText || !node.text) {
          prevLink = null;
          return;
        }

        const linkMark = node.marks.find((m) => m.type === linkType);

        if (linkMark) {
          const textLooksLikeUrl =
            node.text.startsWith("https://") || node.text.startsWith("http://");

          if (textLooksLikeUrl && !isValidUrl(node.text)) {
            if (!tr) tr = newState.tr;
            tr.removeMark(pos, pos + node.text.length, linkType);
            prevLink = null;
          } else if (node.text === linkMark.attrs.href) {
            prevLink = {
              startPos: pos,
              endPos: pos + node.text.length,
              mark: linkMark,
            };
          } else if (textLooksLikeUrl) {
            const updatedMark = linkType.create({
              ...linkMark.attrs,
              href: node.text,
            });
            if (!tr) tr = newState.tr;
            tr.removeMark(pos, pos + node.text.length, linkType);
            tr.addMark(pos, pos + node.text.length, updatedMark);
            prevLink = {
              startPos: pos,
              endPos: pos + node.text.length,
              mark: updatedMark,
            };
          } else {
            prevLink = null;
          }
        } else if (prevLink && pos === prevLink.endPos && node.text) {
          if (!/^\s/.test(node.text[0])) {
            const wsIdx = node.text.search(/\s/);
            const extendLen = wsIdx >= 0 ? wsIdx : node.text.length;
            const newHref =
              prevLink.mark.attrs.href + node.text.slice(0, extendLen);
            if (isValidUrl(newHref)) {
              if (!tr) tr = newState.tr;
              tr.removeMark(prevLink.startPos, prevLink.endPos, linkType);
              tr.addMark(
                prevLink.startPos,
                pos + extendLen,
                linkType.create({ ...prevLink.mark.attrs, href: newHref }),
              );
            }
          }
          prevLink = null;
        } else {
          prevLink = null;
        }
      });

      return tr;
    },
  });
}
