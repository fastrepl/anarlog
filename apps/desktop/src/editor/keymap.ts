import {
  chainCommands,
  createParagraphNear,
  deleteSelection,
  exitCode,
  joinBackward,
  joinForward,
  liftEmptyBlock,
  newlineInCode,
  selectNodeBackward,
  selectNodeForward,
  splitBlock,
  toggleMark,
} from "prosemirror-commands";
import { redo, undo } from "prosemirror-history";
import {
  InputRule,
  inputRules,
  textblockTypeInputRule,
  wrappingInputRule,
} from "prosemirror-inputrules";
import { keymap } from "prosemirror-keymap";
import type { NodeType } from "prosemirror-model";
import {
  liftListItem,
  sinkListItem,
  splitListItem,
} from "prosemirror-schema-list";
import { Selection, type Command, type EditorState } from "prosemirror-state";

import { schema } from "./schema";

function isInListItem(state: EditorState): string | null {
  const { $from } = state.selection;
  for (let depth = $from.depth; depth > 0; depth--) {
    const name = $from.node(depth).type.name;
    if (name === "listItem" || name === "taskItem") return name;
  }
  return null;
}

// ---------------------------------------------------------------------------
// Input rules
// ---------------------------------------------------------------------------
function headingRule(nodeType: NodeType, maxLevel: number) {
  return textblockTypeInputRule(
    new RegExp(`^(#{1,${maxLevel}})\\s$`),
    nodeType,
    (match) => ({ level: match[1].length }),
  );
}

function blockquoteRule(nodeType: NodeType) {
  return wrappingInputRule(/^\s*>\s$/, nodeType);
}

function bulletListRule(nodeType: NodeType) {
  return wrappingInputRule(/^\s*([-+*])\s$/, nodeType);
}

function orderedListRule(nodeType: NodeType) {
  return wrappingInputRule(
    /^\s*(\d+)\.\s$/,
    nodeType,
    (match) => ({ start: +match[1] }),
    (match, node) => node.childCount + node.attrs.start === +match[1],
  );
}

function codeBlockRule(nodeType: NodeType) {
  return textblockTypeInputRule(/^```$/, nodeType);
}

function horizontalRuleRule() {
  return new InputRule(
    /^(?:---|___|\*\*\*)\s$/,
    (state, _match, start, end) => {
      const hr = schema.nodes.horizontalRule.create();
      return state.tr.replaceWith(start - 1, end, [
        hr,
        schema.nodes.paragraph.create(),
      ]);
    },
  );
}

function taskListRule() {
  return new InputRule(/^\s*\[([ x])\]\s$/, (state, match, start, end) => {
    const checked = match[1] === "x";
    const taskItem = schema.nodes.taskItem.create(
      { checked },
      schema.nodes.paragraph.create(),
    );
    const taskList = schema.nodes.taskList.create(null, taskItem);
    return state.tr.replaceWith(start - 1, end, taskList);
  });
}

export function buildInputRules() {
  return inputRules({
    rules: [
      headingRule(schema.nodes.heading, 6),
      blockquoteRule(schema.nodes.blockquote),
      bulletListRule(schema.nodes.bulletList),
      orderedListRule(schema.nodes.orderedList),
      codeBlockRule(schema.nodes.codeBlock),
      horizontalRuleRule(),
      taskListRule(),
    ],
  });
}

// ---------------------------------------------------------------------------
// Keymaps
// ---------------------------------------------------------------------------
const mac =
  typeof navigator !== "undefined"
    ? /Mac|iP(hone|[oa]d)/.test(navigator.platform)
    : false;

export function buildKeymap(onNavigateToTitle?: (pixelWidth?: number) => void) {
  const hardBreak = schema.nodes.hardBreak;

  const keys: Record<string, Command> = {};

  keys["Mod-z"] = undo;
  keys["Mod-Shift-z"] = redo;
  if (!mac) keys["Mod-y"] = redo;

  keys["Mod-b"] = toggleMark(schema.marks.bold);
  keys["Mod-i"] = toggleMark(schema.marks.italic);
  keys["Mod-`"] = toggleMark(schema.marks.code);

  const hardBreakCmd: Command = chainCommands(exitCode, (state, dispatch) => {
    if (dispatch) {
      dispatch(
        state.tr.replaceSelectionWith(hardBreak.create()).scrollIntoView(),
      );
    }
    return true;
  });
  keys["Shift-Enter"] = hardBreakCmd;
  if (mac) keys["Mod-Enter"] = hardBreakCmd;

  keys["Enter"] = chainCommands(
    newlineInCode,
    (state, dispatch) => {
      const itemName = isInListItem(state);
      if (!itemName) return false;
      const { $from } = state.selection;
      if ($from.parent.content.size !== 0) return false;
      const nodeType = state.schema.nodes[itemName];
      if (!nodeType) return false;
      return liftListItem(nodeType)(state, dispatch);
    },
    (state, dispatch) => {
      const itemName = isInListItem(state);
      if (!itemName) return false;
      const nodeType = state.schema.nodes[itemName];
      if (!nodeType) return false;
      return splitListItem(nodeType)(state, dispatch);
    },
    createParagraphNear,
    liftEmptyBlock,
    splitBlock,
  );

  keys["Backspace"] = chainCommands(
    deleteSelection,
    (state, _dispatch) => {
      const { selection } = state;
      if (selection.$head.pos === 0 && selection.empty) return true;
      return false;
    },
    joinBackward,
    selectNodeBackward,
  );

  keys["Delete"] = chainCommands(
    deleteSelection,
    joinForward,
    selectNodeForward,
  );

  keys["Tab"] = (state, dispatch) => {
    const itemName = isInListItem(state);
    if (!itemName) return false;
    const nodeType = state.schema.nodes[itemName];
    if (!nodeType) return false;
    return sinkListItem(nodeType)(state, dispatch);
  };

  keys["Shift-Tab"] = (state, dispatch) => {
    const itemName = isInListItem(state);
    if (!itemName) {
      if (onNavigateToTitle) {
        const { $from } = state.selection;
        const firstBlock = state.doc.firstChild;
        if (firstBlock && $from.start($from.depth) <= 2) {
          onNavigateToTitle();
          return true;
        }
      }
      return false;
    }
    const nodeType = state.schema.nodes[itemName];
    if (!nodeType) return false;
    return liftListItem(nodeType)(state, dispatch);
  };

  if (onNavigateToTitle) {
    keys["ArrowLeft"] = (state) => {
      const { $head, empty } = state.selection;
      if (!empty) return false;
      if ($head.pos !== Selection.atStart(state.doc).from) return false;

      onNavigateToTitle();
      return true;
    };

    keys["ArrowUp"] = (state, _dispatch, view) => {
      const { $head } = state.selection;
      const firstBlockStart = Selection.atStart(state.doc).from;
      if (
        $head.start($head.depth) !==
        state.doc.resolve(firstBlockStart).start($head.depth)
      ) {
        return false;
      }

      if (view) {
        const firstBlock = state.doc.firstChild;
        if (firstBlock && firstBlock.textContent) {
          const text = firstBlock.textContent;
          const posInBlock = $head.pos - $head.start();
          const textBeforeCursor = text.slice(0, posInBlock);
          const firstTextNode = view.dom.querySelector(".ProseMirror > *");
          if (firstTextNode) {
            const style = window.getComputedStyle(firstTextNode);
            const canvas = document.createElement("canvas");
            const ctx = canvas.getContext("2d");
            if (ctx) {
              ctx.font = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
              const pixelWidth = ctx.measureText(textBeforeCursor).width;
              onNavigateToTitle(pixelWidth);
              return true;
            }
          }
        }
      }

      onNavigateToTitle();
      return true;
    };
  }

  return keymap(keys);
}
