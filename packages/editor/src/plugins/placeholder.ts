import { type Node as PMNode, type ResolvedPos } from "prosemirror-model";
import { Plugin, PluginKey } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";

export type PlaceholderFunction = (props: {
  node: PMNode;
  pos: number;
  hasAnchor: boolean;
}) => string;

export const placeholderPluginKey = new PluginKey("placeholder");

function getPlaceholderTarget(doc: PMNode, $anchor: ResolvedPos) {
  for (let depth = $anchor.depth; depth > 0; depth--) {
    const node = $anchor.node(depth);
    if (!node.isLeaf && node.content.size === 0) {
      return {
        pos: $anchor.before(depth),
        node,
      };
    }
  }

  if ($anchor.depth > 0) {
    return null;
  }

  const after = doc.childAfter($anchor.parentOffset);
  if (after.node && !after.node.isLeaf && after.node.content.size === 0) {
    return { pos: after.offset, node: after.node };
  }

  const before = doc.childBefore($anchor.parentOffset);
  if (before.node && !before.node.isLeaf && before.node.content.size === 0) {
    return { pos: before.offset, node: before.node };
  }

  return null;
}

export function placeholderPlugin(placeholder?: PlaceholderFunction) {
  return new Plugin({
    key: placeholderPluginKey,
    props: {
      decorations(state) {
        const { doc, selection } = state;
        const { $anchor } = selection;

        const target = getPlaceholderTarget(doc, $anchor);
        if (!target) {
          return DecorationSet.empty;
        }

        const { pos, node } = target;
        const isEmpty = !node.isLeaf && node.content.size === 0;

        if (!isEmpty) {
          return DecorationSet.empty;
        }

        const isEmptyDoc =
          doc.childCount === 1 &&
          doc.firstChild!.isTextblock &&
          doc.firstChild!.content.size === 0;

        const classes = ["is-empty"];
        if (isEmptyDoc) classes.push("is-editor-empty");

        const text = placeholder
          ? placeholder({ node, pos, hasAnchor: true })
          : "";

        if (!text) {
          return DecorationSet.empty;
        }

        return DecorationSet.create(doc, [
          Decoration.node(pos, pos + node.nodeSize, {
            class: classes.join(" "),
            "data-placeholder": text,
          }),
        ]);
      },
    },
  });
}
