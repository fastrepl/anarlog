import type { Editor } from "@tiptap/core";
import { isNodeActive } from "@tiptap/core";
import { ListKeymap } from "@tiptap/extension-list-keymap";
import type { EditorState } from "@tiptap/pm/state";
import { canJoin } from "@tiptap/pm/transform";

const LIST_ITEM_NAMES = ["taskItem", "listItem"] as const;
type ListItemName = (typeof LIST_ITEM_NAMES)[number];

const LIST_WRAPPER_NAMES: Record<ListItemName, string[]> = {
  taskItem: ["taskList"],
  listItem: ["bulletList", "orderedList"],
};

type JoinRange = {
  from: number;
  to: number;
  joinPos: number;
};

function isListItemName(name: string): name is ListItemName {
  return LIST_ITEM_NAMES.includes(name as ListItemName);
}

function getClosestListItemName(state: EditorState): ListItemName | null {
  const { $from } = state.selection;

  for (let depth = $from.depth; depth > 0; depth--) {
    const nodeName = $from.node(depth).type.name;

    if (isListItemName(nodeName) && state.schema.nodes[nodeName]) {
      return nodeName;
    }
  }

  return null;
}

export function getSelectedListItemNames(state: EditorState): ListItemName[] {
  const matchedNames = new Set<ListItemName>();
  const { selection, doc, schema } = state;
  const { from, to, $from, $to } = selection;

  const addAncestorMatches = (resolvedPos: typeof $from) => {
    for (let depth = resolvedPos.depth; depth > 0; depth--) {
      const nodeName = resolvedPos.node(depth).type.name;

      if (isListItemName(nodeName) && schema.nodes[nodeName]) {
        matchedNames.add(nodeName);
      }
    }
  };

  addAncestorMatches($from);
  addAncestorMatches($to);

  doc.nodesBetween(from, to, (node) => {
    if (node.isText) {
      return;
    }

    const nodeName = node.type.name;
    if (isListItemName(nodeName) && schema.nodes[nodeName]) {
      matchedNames.add(nodeName);
    }
  });

  return LIST_ITEM_NAMES.filter((nodeName) => matchedNames.has(nodeName));
}

export function isSelectionInListItem(state: EditorState): boolean {
  return getSelectedListItemNames(state).length > 0;
}

function joinSeparatedListOnce(
  editor: Editor,
  listItemName: ListItemName,
): boolean {
  const { state } = editor;
  const { doc, schema, selection } = state;
  const paragraphType = schema.nodes.paragraph;
  const listWrapperTypes = LIST_WRAPPER_NAMES[listItemName]
    .map((wrapperName) => schema.nodes[wrapperName])
    .filter(Boolean);

  if (!paragraphType || listWrapperTypes.length === 0) {
    return false;
  }

  let joinRange: JoinRange | undefined;

  doc.nodesBetween(selection.from, selection.to, (node, pos) => {
    if (joinRange || node.type !== paragraphType || node.content.size !== 0) {
      return;
    }

    const $before = doc.resolve(pos);
    const nodeBefore = $before.nodeBefore;
    const $after = doc.resolve(pos + node.nodeSize);
    const nodeAfter = $after.nodeAfter;

    if (
      !nodeBefore ||
      !nodeAfter ||
      nodeBefore.type !== nodeAfter.type ||
      !listWrapperTypes.includes(nodeBefore.type)
    ) {
      return;
    }

    joinRange = {
      from: pos,
      to: pos + node.nodeSize,
      joinPos: pos,
    };

    return false;
  });

  if (joinRange === undefined) {
    return false;
  }

  const { from, to, joinPos } = joinRange;

  return editor
    .chain()
    .command(({ tr }) => {
      tr.delete(from, to);
      if (canJoin(tr.doc, joinPos)) {
        tr.join(joinPos);
      }
      return true;
    })
    .run();
}

function joinSeparatedLists(
  editor: Editor,
  listItemName: ListItemName,
): boolean {
  let joined = false;

  while (joinSeparatedListOnce(editor, listItemName)) {
    joined = true;
  }

  return joined;
}

function runListCommand(
  editor: Editor,
  command: "sinkListItem" | "liftListItem",
): boolean {
  const runCommand = (listItemName: ListItemName) =>
    command === "sinkListItem"
      ? editor.chain().sinkListItem(listItemName).run()
      : editor.chain().liftListItem(listItemName).run();

  for (const listItemName of getSelectedListItemNames(editor.state)) {
    if (runCommand(listItemName)) {
      return true;
    }

    if (joinSeparatedLists(editor, listItemName) && runCommand(listItemName)) {
      return true;
    }
  }

  return false;
}

export function sinkSelectedListItems(editor: Editor): boolean {
  return runListCommand(editor, "sinkListItem");
}

export function liftSelectedListItems(editor: Editor): boolean {
  return runListCommand(editor, "liftListItem");
}

export const CustomListKeymap = ListKeymap.extend({
  addKeyboardShortcuts() {
    const originalShortcuts = this.parent?.() ?? {};

    const tryJoinLists = (editor: typeof this.editor): boolean => {
      const { state } = editor;
      const { selection, doc, schema } = state;
      const { $from } = selection;

      if (!selection.empty || $from.parentOffset !== 0) {
        return false;
      }

      const listWrapperTypes = [
        schema.nodes.taskList,
        schema.nodes.orderedList,
        schema.nodes.bulletList,
      ].filter(Boolean);

      if (listWrapperTypes.length === 0) {
        return false;
      }

      const isListType = (type: (typeof listWrapperTypes)[number]) =>
        listWrapperTypes.includes(type);

      const currentNode = $from.parent;
      const isEmptyParagraph =
        currentNode.type === schema.nodes.paragraph &&
        currentNode.content.size === 0;

      if (isEmptyParagraph) {
        const posBefore = $from.before();
        const posAfter = $from.after();
        const $pos = doc.resolve(posBefore);
        const nodeBefore = $pos.nodeBefore;
        const $posAfter = doc.resolve(posAfter);
        const nodeAfter = $posAfter.nodeAfter;

        if (!nodeBefore || !nodeAfter) {
          return false;
        }

        if (isListType(nodeBefore.type) && nodeBefore.type === nodeAfter.type) {
          const from = posBefore;
          const to = posAfter;
          const joinPos = posBefore;

          editor
            .chain()
            .focus()
            .command(({ tr }) => {
              tr.delete(from, to);
              if (canJoin(tr.doc, joinPos)) {
                tr.join(joinPos);
              }
              return true;
            })
            .run();
          return true;
        }
      }

      for (let depth = $from.depth; depth > 0; depth--) {
        const node = $from.node(depth);
        const isListWrapper = isListType(node.type);

        if (isListWrapper) {
          const indexInParent = $from.index(depth - 1);
          if (indexInParent === 0) {
            continue;
          }

          const posBeforeList = $from.before(depth);
          const $posBeforeList = doc.resolve(posBeforeList);
          const nodeBefore = $posBeforeList.nodeBefore;

          if (nodeBefore && nodeBefore.type === node.type) {
            if (canJoin(doc, posBeforeList)) {
              editor
                .chain()
                .focus()
                .command(({ tr }) => {
                  tr.join(posBeforeList);
                  return true;
                })
                .run();
              return true;
            }
          }
          break;
        }
      }

      return false;
    };

    return {
      ...originalShortcuts,

      Enter: () => {
        const editor = this.editor;
        const state = editor.state;
        const { selection } = state;
        const listItemName = getClosestListItemName(state);

        if (!listItemName) {
          return false;
        }

        if (
          isNodeActive(state, listItemName) &&
          selection.$from.parent.content.size === 0
        ) {
          return editor.chain().liftListItem(listItemName).run();
        }

        return originalShortcuts.Enter
          ? originalShortcuts.Enter({ editor })
          : false;
      },

      Backspace: ({ editor }) => {
        const state = editor.state;
        const { selection } = state;
        const listItemName = getClosestListItemName(state);

        if (listItemName) {
          if (
            isNodeActive(state, listItemName) &&
            selection.$from.parentOffset === 0 &&
            selection.$from.parent.content.size === 0
          ) {
            return editor.chain().liftListItem(listItemName).run();
          }
        }

        if (tryJoinLists(editor)) {
          return true;
        }

        if (originalShortcuts.Backspace) {
          return originalShortcuts.Backspace({ editor });
        }

        return false;
      },

      Tab: () => {
        return sinkSelectedListItems(this.editor);
      },

      "Shift-Tab": () => {
        return liftSelectedListItems(this.editor);
      },
    };
  },
});

export default CustomListKeymap;
