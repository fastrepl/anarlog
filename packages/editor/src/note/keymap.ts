import {
  chainCommands,
  createParagraphNear,
  deleteSelection,
  joinBackward,
  joinForward,
  liftEmptyBlock,
  newlineInCode,
  selectAll,
  selectNodeBackward,
  selectNodeForward,
  selectTextblockEnd,
  selectTextblockStart,
  setBlockType,
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
import { Fragment, type MarkType, type NodeType } from "prosemirror-model";
import {
  liftListItem,
  sinkListItem,
  splitListItem,
} from "prosemirror-schema-list";
import {
  Selection,
  TextSelection,
  type Command,
  type EditorState,
  type Transaction,
} from "prosemirror-state";

import { createTaskItemAttrs } from "../tasks";
import { schema } from "./schema";

function isInListItem(state: EditorState): string | null {
  const { $from } = state.selection;
  for (let depth = $from.depth; depth > 0; depth--) {
    const name = $from.node(depth).type.name;
    if (name === "listItem" || name === "taskItem") return name;
  }
  return null;
}

function moveListItem(direction: "up" | "down"): Command {
  return (state, dispatch) => {
    const { $from } = state.selection;

    let depth = -1;
    for (let d = $from.depth; d > 0; d--) {
      const name = $from.node(d).type.name;
      if (name === "listItem" || name === "taskItem") {
        depth = d;
        break;
      }
    }
    if (depth === -1) return false;

    const parent = $from.node(depth - 1);
    const index = $from.index(depth - 1);
    const atBoundary =
      direction === "up" ? index === 0 : index >= parent.childCount - 1;

    if (!atBoundary) {
      // Swap with adjacent sibling
      const siblingIndex = direction === "up" ? index - 1 : index + 1;
      const currentItem = parent.child(index);
      const siblingItem = parent.child(siblingIndex);

      if (dispatch) {
        const tr = state.tr;
        const currentStart = $from.before(depth);
        const currentEnd = $from.after(depth);

        if (direction === "up") {
          const prevStart = currentStart - siblingItem.nodeSize;
          tr.replaceWith(
            prevStart,
            currentEnd,
            Fragment.from([currentItem, siblingItem]),
          );
          const offset = prevStart - currentStart;
          tr.setSelection(
            TextSelection.create(
              tr.doc,
              state.selection.anchor + offset,
              state.selection.head + offset,
            ),
          );
        } else {
          const nextEnd = currentEnd + siblingItem.nodeSize;
          tr.replaceWith(
            currentStart,
            nextEnd,
            Fragment.from([siblingItem, currentItem]),
          );
          const offset = siblingItem.nodeSize;
          tr.setSelection(
            TextSelection.create(
              tr.doc,
              state.selection.anchor + offset,
              state.selection.head + offset,
            ),
          );
        }

        dispatch(tr.scrollIntoView());
      }
      return true;
    }

    // At boundary: lift item into the outer (parent) list
    let outerDepth = -1;
    for (let d = depth - 2; d > 0; d--) {
      const name = $from.node(d).type.name;
      if (name === "listItem" || name === "taskItem") {
        outerDepth = d;
        break;
      }
    }
    if (outerDepth === -1) return false;

    // Only lift when the item type is compatible with the outer list
    const outerListName = $from.node(outerDepth - 1).type.name;
    const currentItemName = $from.node(depth).type.name;
    const compatible =
      (currentItemName === "listItem" &&
        (outerListName === "bulletList" || outerListName === "orderedList")) ||
      (currentItemName === "taskItem" && outerListName === "taskList");
    if (!compatible) return false;

    if (dispatch) {
      const tr = state.tr;
      const currentItem = parent.child(index);
      const currentStart = $from.before(depth);
      const currentEnd = $from.after(depth);
      const anchorOffset = state.selection.anchor - currentStart;

      // Delete the item, or the entire nested list when it's the only child
      if (parent.childCount === 1) {
        tr.delete($from.before(depth - 1), $from.after(depth - 1));
      } else {
        tr.delete(currentStart, currentEnd);
      }

      // Insert into the outer list: before the outer item (up) or after (down)
      const targetPos =
        direction === "up" ? $from.before(outerDepth) : $from.after(outerDepth);
      const insertPos = tr.mapping.map(targetPos);
      tr.insert(insertPos, currentItem);

      tr.setSelection(TextSelection.create(tr.doc, insertPos + anchorOffset));
      dispatch(tr.scrollIntoView());
    }
    return true;
  };
}

function joinTaskItemBackward(
  state: EditorState,
  dispatch?: (tr: Transaction) => void,
) {
  const { selection } = state;
  const { $from } = selection;

  if (!selection.empty || $from.parentOffset !== 0) {
    return false;
  }

  let itemDepth = -1;
  for (let depth = $from.depth; depth > 0; depth--) {
    if ($from.node(depth).type === schema.nodes.taskItem) {
      itemDepth = depth;
      break;
    }
  }

  if (itemDepth === -1 || $from.parent.type !== schema.nodes.paragraph) {
    return false;
  }

  const list = $from.node(itemDepth - 1);
  if (list.type !== schema.nodes.taskList) {
    return false;
  }

  if ($from.index(itemDepth) !== 0) {
    return false;
  }

  const itemIndex = $from.index(itemDepth - 1);
  const currentItem = list.child(itemIndex);
  if (itemIndex === 0) {
    // Lift instead of falling through to joinBackward, which would merge the
    // item's text into the preceding block and silently destroy the task
    if (!currentItem.firstChild?.content.size) return false;
    return liftListItem(schema.nodes.taskItem)(state, dispatch);
  }

  const previousItem = list.child(itemIndex - 1);
  const previousParagraphIndex = previousItem.childCount - 1;
  const previousParagraph = previousItem.child(previousParagraphIndex);
  const currentParagraph = currentItem.firstChild;

  if (
    previousParagraph.type !== schema.nodes.paragraph ||
    currentParagraph?.type !== schema.nodes.paragraph
  ) {
    return false;
  }

  if (!dispatch) {
    return true;
  }

  const mergedParagraph = previousParagraph.type.create(
    previousParagraph.attrs,
    previousParagraph.content.append(currentParagraph.content),
    previousParagraph.marks,
  );

  const mergedPreviousContent = [
    ...Array.from({ length: previousParagraphIndex }, (_, index) =>
      previousItem.child(index),
    ),
    mergedParagraph,
    ...Array.from({ length: currentItem.childCount - 1 }, (_, index) =>
      currentItem.child(index + 1),
    ),
  ];
  const mergedPreviousItem = previousItem.type.create(
    previousItem.attrs,
    Fragment.from(mergedPreviousContent),
    previousItem.marks,
  );

  const currentStart = $from.before(itemDepth);
  const currentEnd = $from.after(itemDepth);
  const previousStart = currentStart - previousItem.nodeSize;
  let paragraphOffset = 0;
  for (let index = 0; index < previousParagraphIndex; index++) {
    paragraphOffset += previousItem.child(index).nodeSize;
  }
  const selectionPos =
    previousStart + 1 + paragraphOffset + 1 + previousParagraph.content.size;

  const tr = state.tr.replaceWith(
    previousStart,
    currentEnd,
    mergedPreviousItem,
  );
  tr.setSelection(TextSelection.create(tr.doc, selectionPos));
  dispatch(tr.scrollIntoView());
  return true;
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

function markInputRule(pattern: RegExp, markType: MarkType, delimLen: number) {
  return new InputRule(pattern, (state, match, start, end) => {
    const prefix = match[1];
    const content = match[2];
    const { tr } = state;

    const openStart = start + prefix.length;
    // The typed character that triggered this rule is the last char of
    // the closing delimiter and is NOT in the document yet.  Only the
    // remaining delimLen-1 chars need to be removed.
    const closeCharsInDoc = delimLen - 1;

    const $start = state.doc.resolve(openStart);
    if (!$start.parent.type.allowsMarkType(markType)) return null;

    if (closeCharsInDoc > 0) {
      tr.delete(end - closeCharsInDoc, end);
    }
    tr.delete(openStart, openStart + delimLen);
    tr.addMark(openStart, openStart + content.length, markType.create());
    tr.removeStoredMark(markType);

    return tr;
  });
}

function isInCodeInputContext(state: EditorState) {
  const { $from } = state.selection;

  for (let depth = $from.depth; depth > 0; depth--) {
    if ($from.node(depth).type.spec.code) {
      return true;
    }
  }

  return Boolean(schema.marks.code.isInSet(state.storedMarks ?? $from.marks()));
}

function textReplacementRule(pattern: RegExp, replacement: string) {
  return new InputRule(pattern, (state, _match, start, end) => {
    if (isInCodeInputContext(state)) {
      return null;
    }

    return state.tr.insertText(replacement, start, end);
  });
}

// Char-style typographic replacements. Patterns capture an optional prefix
// (kept) and the token (replaced); a third group is a kept suffix.
const SYMBOL_REPLACEMENTS: Record<string, string> = {
  "->": "→",
  "<-": "←",
  "<->": "↔",
  "==>": "⇒",
  "<==": "⇐",
  "<=>": "⇔",
  "+-": "±",
  "+/-": "±",
  "=/=": "≠",
};

const ABBREVIATION_REPLACEMENTS: Record<string, string> = {
  "(c)": "©",
  "(r)": "®",
  "(tm)": "™",
};

const FRACTION_REPLACEMENTS: Record<string, string> = {
  "c/o": "℅",
  "1/2": "½",
  "1/3": "⅓",
  "1/4": "¼",
  "1/5": "⅕",
  "1/6": "⅙",
  "1/8": "⅛",
  "2/3": "⅔",
  "2/5": "⅖",
  "3/4": "¾",
  "3/5": "⅗",
  "3/8": "⅜",
  "4/5": "⅘",
  "5/6": "⅚",
  "5/8": "⅝",
  "7/8": "⅞",
};

function mappedReplacementRule(pattern: RegExp, map: Record<string, string>) {
  return new InputRule(pattern, (state, match, start, end) => {
    if (isInCodeInputContext(state)) return null;
    const prefix = match[1] ?? "";
    const replacement = map[(match[2] ?? "").toLowerCase()];
    if (!replacement) return null;
    const suffix = match[3] ?? "";
    return state.tr.insertText(
      replacement + suffix,
      start + prefix.length,
      end,
    );
  });
}

function symbolReplacementRule() {
  return new InputRule(
    /(?:<->|==>|<==|<=>|->|<-|\+\/-|\+-|=\/=)$/,
    (state, match, start, end) => {
      if (isInCodeInputContext(state)) return null;
      const replacement = SYMBOL_REPLACEMENTS[match[0].toLowerCase()];
      if (!replacement) return null;
      return state.tr.insertText(replacement, start, end);
    },
  );
}

// The preceding character must not be a dash, so a line-start `---` survives
// long enough for horizontalRuleRule to claim it on the following space.
function dashReplacementRule() {
  return new InputRule(/([^-])--$/, (state, match, start, end) => {
    if (isInCodeInputContext(state)) return null;
    const prefix = match[1] ?? "";
    return state.tr.insertText("—", start + prefix.length, end);
  });
}

// Same semantics as prosemirror-inputrules smartQuotes/ellipsis, but guarded
// so quotes inside code stay straight.
function quoteRule(pattern: RegExp, replacement: string) {
  return new InputRule(pattern, (state, match, start, end) => {
    if (isInCodeInputContext(state)) return null;
    let insertStart = start;
    if (match.length > 1 && typeof match[1] === "string") {
      insertStart = start + match[0].lastIndexOf(match[1]);
    }
    return state.tr.insertText(replacement, insertStart, end);
  });
}

function taskListRule() {
  return new InputRule(/^\s*\[([ x]?)\]\s$/, (state, match, start, end) => {
    const checked = match[1] === "x";
    const taskItem = schema.nodes.taskItem.create(
      createTaskItemAttrs(checked),
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
      symbolReplacementRule(),
      dashReplacementRule(),
      mappedReplacementRule(
        /(^|[\s([{])(\((?:c|r|tm)\))$/i,
        ABBREVIATION_REPLACEMENTS,
      ),
      mappedReplacementRule(
        /(^|[\s([{])((?:c\/o|1\/2|1\/3|1\/4|1\/5|1\/6|1\/8|2\/3|2\/5|3\/4|3\/5|3\/8|4\/5|5\/6|5\/8|7\/8))([\s.,;:!?])$/i,
        FRACTION_REPLACEMENTS,
      ),
      quoteRule(/(?:^|[\s{[(<'"‘“])(")$/, "“"),
      quoteRule(/"$/, "”"),
      quoteRule(/(?:^|[\s{[(<'"‘“])(')$/, "‘"),
      quoteRule(/'$/, "’"),
      textReplacementRule(/\.\.\.$/, "…"),
      markInputRule(/(^|[^*])\*\*([^*]+)\*\*$/, schema.marks.bold, 2),
      markInputRule(/(^|[^~])~~([^~]+)~~$/, schema.marks.strike, 2),
      markInputRule(/(^|[^=])==([^=]+)==$/, schema.marks.highlight, 2),
      markInputRule(/(^|[^*])\*([^*]+)\*$/, schema.marks.italic, 1),
      markInputRule(/(^|[^_])_([^_]+)_$/, schema.marks.italic, 1),
      markInputRule(/(^|[^~])~([^~]+)~$/, schema.marks.strike, 1),
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
  const keys: Record<string, Command> = {};

  keys["Mod-z"] = undo;
  keys["Mod-Shift-z"] = redo;
  if (!mac) keys["Mod-y"] = redo;

  keys["Mod-b"] = toggleMark(schema.marks.bold);
  keys["Mod-i"] = toggleMark(schema.marks.italic);
  keys["Mod-u"] = toggleMark(schema.marks.underline);
  keys["Mod-`"] = toggleMark(schema.marks.code);

  const exitCodeBlockOnEmptyLine: Command = (state, dispatch) => {
    const { $from } = state.selection;
    if (!$from.parent.type.spec.code) return false;

    const lastLine = $from.parent.textContent.split("\n").pop() ?? "";
    const atEnd = $from.parentOffset === $from.parent.content.size;
    if (!atEnd || lastLine !== "") return false;

    if (dispatch) {
      const codeBlockPos = $from.before($from.depth);
      const codeBlock = $from.parent;
      const textContent = codeBlock.textContent.replace(/\n$/, "");
      const tr = state.tr;

      tr.replaceWith(
        codeBlockPos,
        codeBlockPos + codeBlock.nodeSize,
        textContent
          ? [
              schema.nodes.codeBlock.create(null, schema.text(textContent)),
              schema.nodes.paragraph.create(),
            ]
          : [schema.nodes.paragraph.create()],
      );

      const newParaPos = textContent
        ? codeBlockPos + textContent.length + 2 + 1
        : codeBlockPos + 1;
      tr.setSelection(TextSelection.create(tr.doc, newParaPos));
      dispatch(tr.scrollIntoView());
    }
    return true;
  };

  keys["Enter"] = chainCommands(
    exitCodeBlockOnEmptyLine,
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

  const revertBlockToParagraph: Command = (state, dispatch) => {
    const { $from } = state.selection;
    if (!state.selection.empty || $from.parentOffset !== 0) return false;
    const node = $from.parent;
    if (
      node.type !== schema.nodes.heading &&
      node.type !== schema.nodes.codeBlock
    ) {
      return false;
    }
    return setBlockType(schema.nodes.paragraph)(state, dispatch);
  };

  const backspaceCmd: Command = chainCommands(
    deleteSelection,
    (state, _dispatch) => {
      const { selection } = state;
      if (selection.$head.pos === 0 && selection.empty) return true;
      return false;
    },
    revertBlockToParagraph,
    joinTaskItemBackward,
    joinBackward,
    selectNodeBackward,
  );
  keys["Backspace"] = backspaceCmd;
  keys["Mod-Backspace"] = backspaceCmd;
  keys["Shift-Backspace"] = backspaceCmd;

  const deleteCmd: Command = chainCommands(
    deleteSelection,
    joinForward,
    selectNodeForward,
  );
  keys["Delete"] = deleteCmd;
  keys["Mod-Delete"] = deleteCmd;

  keys["Mod-a"] = selectAll;

  if (mac) {
    keys["Ctrl-h"] = backspaceCmd;
    keys["Alt-Backspace"] = backspaceCmd;
    keys["Ctrl-d"] = deleteCmd;
    keys["Ctrl-Alt-Backspace"] = deleteCmd;
    keys["Alt-Delete"] = deleteCmd;
    keys["Alt-d"] = deleteCmd;
    keys["Ctrl-a"] = selectTextblockStart;
    keys["Ctrl-e"] = selectTextblockEnd;
  }

  // Prevent Tab from moving focus outside the editor
  keys["Tab"] = (state, dispatch) => {
    const itemName = isInListItem(state);
    if (!itemName) return true;
    const nodeType = state.schema.nodes[itemName];
    if (!nodeType) return true;
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

  keys["Alt-ArrowUp"] = moveListItem("up");
  keys["Alt-ArrowDown"] = moveListItem("down");

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
