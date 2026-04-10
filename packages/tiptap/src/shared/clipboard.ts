import {
  Extension,
  getTextBetween,
  getTextSerializersFromSchema,
} from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";

export const ClipboardTextSerializer = Extension.create({
  name: "charClipboardTextSerializer",

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey("charClipboardTextSerializer"),
        props: {
          clipboardTextSerializer: () => {
            const { editor } = this;
            const { state, schema } = editor;
            const { doc, selection } = state;
            const { ranges } = selection;
            const from = Math.min(...ranges.map((range) => range.$from.pos));
            const to = Math.max(...ranges.map((range) => range.$to.pos));

            if (from === 0 && to === doc.content.size) {
              return editor.getMarkdown();
            }

            const textSerializers = getTextSerializersFromSchema(schema);
            const range = { from, to };

            const text = getTextBetween(doc, range, {
              textSerializers,
            });
            return text;
          },
        },
      }),
    ];
  },
});
