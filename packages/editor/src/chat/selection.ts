import { Fragment, type Node as PMNode, Slice } from "prosemirror-model";
import type { EditorView } from "prosemirror-view";

// As much text before the caret as dictation's fitting and cleanup look at.
const BEFORE_WINDOW = 1_000;

/**
 * The caret or selection as it was at one moment, and the one write that may land exactly
 * there. Dictation fits its text to `before` and may rewrite `selected` by voice.
 */
export interface ChatEditorSelection {
  /** Up to 1,000 characters before the selection, with "\n" between paragraphs. */
  readonly before: string;
  /**
   * The selected text, or null when nothing is selected or the selection holds a mention or
   * an attachment, which plain text can't put back.
   */
  readonly selected: string | null;
  /** Whether the selection is only a caret. */
  readonly empty: boolean;
  /**
   * Types `text` over the selection in one transaction, and with `join` also over the period
   * right before a caret. Types nothing, and returns false, unless the same editor still holds
   * exactly the content and selection that were read.
   */
  replace(
    text: string,
    options?: { join?: boolean; separateFollowingWord?: boolean },
  ): boolean;
}

export function readChatSelection(
  view: EditorView,
  currentView: () => EditorView | null,
): ChatEditorSelection {
  const { doc, selection } = view.state;
  const { from, to, empty } = selection;
  const before = doc
    .textBetween(Math.max(0, from - BEFORE_WINDOW), from, "\n", leafText)
    // The window may start inside a surrogate pair.
    .replace(/^[\uDC00-\uDFFF]/u, "");

  return {
    before,
    selected:
      empty || holdsAtom(doc, from, to)
        ? null
        : doc.textBetween(from, to, "\n", leafText),
    empty,
    replace(text, { join = false, separateFollowingWord = false } = {}) {
      if (view.isDestroyed || currentView() !== view) return false;
      const { state } = view;
      if (
        !state.doc.eq(doc) ||
        state.selection.from !== from ||
        state.selection.to !== to
      )
        return false;
      const start = join ? from - 1 : from;
      if (join && (!empty || state.doc.textBetween(start, from) !== "."))
        return false;
      const after = state.selection.$to.nodeAfter?.text ?? "";
      if (
        separateFollowingWord &&
        /\S$/u.test(text) &&
        /^[\p{L}\p{N}]/u.test(after) &&
        !/[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}]$/u.test(
          text,
        ) &&
        !/^[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}]/u.test(after)
      )
        text += " ";
      view.dispatch(state.tr.replace(start, to, textSlice(state.schema, text)));
      return true;
    },
  };
}

function leafText(node: PMNode) {
  if (node.type.name === "hardBreak") return "\n";
  if (node.type.name === "mention-@") return `@${node.attrs.label ?? ""}`;
  return "\ufffc";
}

function holdsAtom(doc: PMNode, from: number, to: number) {
  let found = false;
  doc.nodesBetween(from, to, (node) => {
    if (node.isInline && !node.isText && node.type.name !== "hardBreak")
      found = true;
    return !found;
  });
  return found;
}

/** Plain text as chat paragraphs, one per line. */
function textSlice(schema: EditorView["state"]["schema"], text: string) {
  const paragraphs = text
    .split(/\r?\n/u)
    .map((line) =>
      schema.nodes.paragraph.create(null, line ? schema.text(line) : null),
    );
  return new Slice(Fragment.from(paragraphs), 1, 1);
}
