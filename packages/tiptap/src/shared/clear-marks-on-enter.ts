import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";

const INLINE_MARK_NAMES = ["bold", "italic", "underline"];

export const ClearMarksOnEnter = Extension.create({
  name: "clearMarksOnEnter",

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey("clearMarksOnEnter"),
        appendTransaction(transactions, oldState, newState) {
          if (!transactions.some((tr) => tr.docChanged)) return null;
          if (newState.doc.content.size <= oldState.doc.content.size)
            return null;

          const { $head } = newState.selection;
          const currentNode = $head.parent;

          if (
            currentNode.type.name !== "paragraph" ||
            currentNode.content.size !== 0 ||
            $head.parentOffset !== 0
          ) {
            return null;
          }

          const storedMarks = newState.storedMarks;
          if (!storedMarks || storedMarks.length === 0) return null;

          const filtered = storedMarks.filter(
            (mark) => !INLINE_MARK_NAMES.includes(mark.type.name),
          );

          if (filtered.length === storedMarks.length) return null;

          return newState.tr.setStoredMarks(filtered);
        },
      }),
    ];
  },
});
