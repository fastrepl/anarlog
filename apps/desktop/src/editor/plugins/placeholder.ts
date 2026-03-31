import { type Node as PMNode } from "prosemirror-model";
import { Plugin, PluginKey } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";

export type PlaceholderFunction = (props: {
  node: PMNode;
  pos: number;
  hasAnchor: boolean;
}) => string;

export const placeholderPluginKey = new PluginKey("placeholder");

export function placeholderPlugin(placeholder?: PlaceholderFunction) {
  return new Plugin({
    key: placeholderPluginKey,
    props: {
      decorations(state) {
        const { doc, selection } = state;
        const { anchor } = selection;
        const decorations: Decoration[] = [];

        const isEmptyDoc =
          doc.childCount === 1 &&
          doc.firstChild!.isTextblock &&
          doc.firstChild!.content.size === 0;

        doc.descendants((node, pos) => {
          const hasAnchor = anchor >= pos && anchor <= pos + node.nodeSize;
          const isEmpty = !node.isLeaf && node.content.size === 0;

          if (hasAnchor && isEmpty) {
            const classes = ["is-empty"];
            if (isEmptyDoc) classes.push("is-editor-empty");

            const text = placeholder
              ? placeholder({ node, pos, hasAnchor })
              : "";

            if (text) {
              decorations.push(
                Decoration.node(pos, pos + node.nodeSize, {
                  class: classes.join(" "),
                  "data-placeholder": text,
                }),
              );
            }
          }

          return false;
        });

        return DecorationSet.create(doc, decorations);
      },
    },
  });
}
