import { type Node as PMNode } from "prosemirror-model";
import { Plugin, PluginKey } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";

export type PlaceholderFunction = (props: {
  node: PMNode;
  pos: number;
  parent: PMNode | null;
  hasAnchor: boolean;
}) => string;

export const placeholderPluginKey = new PluginKey("placeholder");

export function placeholderPlugin(placeholder?: PlaceholderFunction) {
  return new Plugin({
    key: placeholderPluginKey,
    props: {
      decorations(state) {
        const { doc, selection } = state;
        const { anchor, $anchor } = selection;
        const decorations: Decoration[] = [];

        const isEmptyDoc =
          doc.childCount === 1 &&
          doc.firstChild!.isTextblock &&
          doc.firstChild!.content.size === 0;

        doc.descendants((node, pos, parent) => {
          const hasAnchor = anchor >= pos && anchor <= pos + node.nodeSize;
          const isEmpty = !node.isLeaf && node.content.size === 0;

          if (hasAnchor && isEmpty) {
            const classes = ["is-empty"];
            if (isEmptyDoc) classes.push("is-editor-empty");

            const text = placeholder
              ? placeholder({ node, pos, parent, hasAnchor })
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

        if (placeholder && $anchor.depth > 1) {
          const node = $anchor.parent;
          if (!node.isLeaf && node.content.size === 0) {
            const parent = $anchor.node($anchor.depth - 1);
            const pos = $anchor.before($anchor.depth);
            const text = placeholder({
              node,
              pos,
              parent,
              hasAnchor: true,
            });

            if (text) {
              decorations.push(
                Decoration.widget(
                  pos + 1,
                  () => {
                    const span = document.createElement("span");
                    span.className = "is-empty";
                    span.setAttribute("data-placeholder", text);
                    return span;
                  },
                  { side: 0, key: `placeholder-${pos}-${text}` },
                ),
              );
            }
          }
        }

        return DecorationSet.create(doc, decorations);
      },
    },
  });
}
