import { type Node as PMNode, type ResolvedPos } from "prosemirror-model";
import { Plugin, PluginKey } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";

export type PlaceholderFunction = (props: {
  node: PMNode;
  pos: number;
  hasAnchor: boolean;
}) => string;

export type PersistentPlaceholderFunction = (props: {
  doc: PMNode;
  node: PMNode;
  pos: number;
}) => string;

export const placeholderPluginKey = new PluginKey("placeholder");

export const PLACEHOLDER_COMPOSING_CLASS = "is-composing";

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

export function placeholderPlugin(
  placeholder?: PlaceholderFunction,
  persistentPlaceholder?: PersistentPlaceholderFunction,
) {
  return new Plugin({
    key: placeholderPluginKey,
    props: {
      handleDOMEvents: {
        compositionstart(view) {
          view.dom.classList.add(PLACEHOLDER_COMPOSING_CLASS);
          return false;
        },
        compositionend(view) {
          view.dom.classList.remove(PLACEHOLDER_COMPOSING_CLASS);
          return false;
        },
      },
      decorations(state) {
        const { doc, selection } = state;
        const { $anchor } = selection;

        const target = getPlaceholderTarget(doc, $anchor);
        const isEmptyDoc =
          doc.childCount === 1 &&
          doc.firstChild!.isTextblock &&
          doc.firstChild!.content.size === 0;
        const decorations: Decoration[] = [];
        const decoratedPositions = new Set<number>();

        const addDecoration = (node: PMNode, pos: number, text: string) => {
          if (!text || node.isLeaf || node.content.size > 0) {
            return;
          }

          const classes = ["is-empty"];
          if (isEmptyDoc) classes.push("is-editor-empty");
          decorations.push(
            Decoration.node(pos, pos + node.nodeSize, {
              class: classes.join(" "),
              "data-placeholder": text,
            }),
          );
          decoratedPositions.add(pos);
        };

        if (target && placeholder) {
          addDecoration(
            target.node,
            target.pos,
            placeholder({
              node: target.node,
              pos: target.pos,
              hasAnchor: true,
            }),
          );
        }

        if (persistentPlaceholder) {
          doc.descendants((node, pos) => {
            if (decoratedPositions.has(pos)) {
              return;
            }

            addDecoration(node, pos, persistentPlaceholder({ doc, node, pos }));
          });
        }

        return decorations.length > 0
          ? DecorationSet.create(doc, decorations)
          : DecorationSet.empty;
      },
    },
  });
}
