import { get as getEmojiByShortcode } from "node-emoji";
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
  undoInputRule,
  wrappingInputRule,
} from "prosemirror-inputrules";
import { keymap } from "prosemirror-keymap";
import {
  Fragment,
  type MarkType,
  type NodeType,
  type ResolvedPos,
} from "prosemirror-model";
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
import { findWrapping } from "prosemirror-transform";

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
    if (isInCodeInputContext(state)) return null;
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

// `[]`, `[ ]`, `[x]`, or `[X]` followed by a space wraps the paragraph in
// taskList > taskItem wherever the surrounding node allows a taskList. Inside
// a bulletList/orderedList > listItem, the marker converts that listItem into
// a taskItem, splitting the list around it and merging with adjacent
// taskLists when possible. Ported from char's editor taskListRule.
function taskListRule() {
  return new InputRule(/^\s*\[([ xX]?)\]\s$/, (state, match, start, end) => {
    const $start = state.doc.resolve(start);
    const { taskList, taskItem, paragraph, bulletList, orderedList, listItem } =
      schema.nodes;
    if ($start.parent.type !== paragraph) return null;
    const checked = match[1] === "x" || match[1] === "X";
    const taskAttrs = createTaskItemAttrs(checked);

    const listDepth = $start.depth - 2;
    const itemDepth = $start.depth - 1;
    const listNode = listDepth >= 0 ? $start.node(listDepth) : null;
    const itemNode = itemDepth >= 0 ? $start.node(itemDepth) : null;

    // Inside a list item, only when the marker is at the very start
    // (first paragraph of the listItem).
    if (
      itemNode?.type === listItem &&
      listNode &&
      (listNode.type === bulletList || listNode.type === orderedList) &&
      $start.index(itemDepth) === 0
    ) {
      const firstParagraph = itemNode.firstChild;
      if (!firstParagraph || firstParagraph.type !== paragraph) return null;

      // Build the taskItem from the listItem's content, stripping the marker.
      // The handler runs against the doc *before* the typed char is inserted,
      // so the in-doc prefix length is `end - start`.
      const newFirstParagraph = paragraph.create(
        firstParagraph.attrs,
        firstParagraph.content.cut(end - start),
        firstParagraph.marks,
      );
      const taskItemChildren = [newFirstParagraph];
      for (let i = 1; i < itemNode.childCount; i++) {
        taskItemChildren.push(itemNode.child(i));
      }
      const newTaskItem = taskItem.create(
        taskAttrs,
        Fragment.from(taskItemChildren),
      );

      // Partition the list's items around the converted item.
      const itemIndex = $start.index(listDepth);
      const itemsBefore = [];
      const itemsAfter = [];
      for (let i = 0; i < listNode.childCount; i++) {
        if (i < itemIndex) itemsBefore.push(listNode.child(i));
        else if (i > itemIndex) itemsAfter.push(listNode.child(i));
      }

      // If the new taskList would be flush against an existing taskList
      // sibling (no leftover list items on that side), merge into it.
      const listStart = $start.before(listDepth);
      const listEnd = $start.after(listDepth);
      const grandParent = $start.node(listDepth - 1);
      const listIndex = $start.index(listDepth - 1);
      const prevSibling =
        listIndex > 0 ? grandParent.child(listIndex - 1) : null;
      const nextSibling =
        listIndex < grandParent.childCount - 1
          ? grandParent.child(listIndex + 1)
          : null;
      const mergePrev =
        itemsBefore.length === 0 && prevSibling?.type === taskList;
      const mergeNext =
        itemsAfter.length === 0 && nextSibling?.type === taskList;

      // Replace [list (+ adjacent taskList siblings to merge)] with
      // [list_before?, taskList, list_after?].
      let replaceFrom = listStart;
      let replaceTo = listEnd;
      const newNodes = [];

      if (itemsBefore.length > 0) {
        newNodes.push(
          listNode.type.create(listNode.attrs, Fragment.from(itemsBefore)),
        );
      }

      const taskListChildren = [];
      let prevSiblingChildSize = 0;
      if (mergePrev && prevSibling) {
        replaceFrom = listStart - prevSibling.nodeSize;
        for (let i = 0; i < prevSibling.childCount; i++) {
          taskListChildren.push(prevSibling.child(i));
          prevSiblingChildSize += prevSibling.child(i).nodeSize;
        }
      }
      taskListChildren.push(newTaskItem);
      if (mergeNext && nextSibling) {
        replaceTo = listEnd + nextSibling.nodeSize;
        for (let i = 0; i < nextSibling.childCount; i++) {
          taskListChildren.push(nextSibling.child(i));
        }
      }
      newNodes.push(taskList.create(null, Fragment.from(taskListChildren)));

      if (itemsAfter.length > 0) {
        // A trailing ordered list must continue numbering after the
        // converted item, not restart at the original `start`.
        const afterAttrs =
          listNode.type === orderedList
            ? {
                ...listNode.attrs,
                start: listNode.attrs.start + itemIndex + 1,
              }
            : listNode.attrs;
        newNodes.push(
          listNode.type.create(afterAttrs, Fragment.from(itemsAfter)),
        );
      }

      const tr = state.tr.replaceWith(replaceFrom, replaceTo, newNodes);

      // Place the cursor at the start of the new taskItem's paragraph: skip
      // the leading list (if any), enter taskList, skip any merged-in prior
      // taskItems, then enter taskItem and paragraph.
      let cursor = replaceFrom;
      if (itemsBefore.length > 0) cursor += newNodes[0].nodeSize;
      cursor += 1 + prevSiblingChildSize + 1 + 1;
      tr.setSelection(TextSelection.create(tr.doc, cursor));
      return tr;
    }

    // List-item paragraphs are only handled by the branch above; a marker in
    // a non-leading paragraph of an item should stay literal, not nest a list.
    const parentType = itemNode?.type ?? null;
    if (parentType === listItem || parentType === taskItem) return null;

    // Anywhere else — wrap the paragraph in taskList > taskItem when the
    // surrounding node allows it (findWrapping rejects e.g. list items).
    const tr = state.tr.delete(start, end);
    const range = tr.doc.resolve(start).blockRange();
    if (!range) return null;
    const wrapping = findWrapping(range, taskList);
    if (!wrapping) return null;
    tr.wrap(
      range,
      wrapping.map((wrapper) =>
        wrapper.type === taskItem
          ? { type: wrapper.type, attrs: taskAttrs }
          : wrapper,
      ),
    );
    return tr;
  });
}

function underlineRule() {
  return new InputRule(/<u>([^<]+)<\/u>$/i, (state, match, start, end) => {
    if (isInCodeInputContext(state)) return null;
    const content = match[1];
    if (!content) return null;
    const contentStart = start + "<u>".length;
    const contentEnd = contentStart + content.length;
    const markType = schema.marks.underline;
    if (!state.doc.resolve(contentStart).parent.type.allowsMarkType(markType)) {
      return null;
    }
    const tr = state.tr;
    if (end > contentEnd) tr.delete(contentEnd, end);
    tr.addMark(contentStart, contentEnd, markType.create());
    tr.delete(start, contentStart);
    tr.removeStoredMark(markType);
    return tr;
  });
}

function emojiReplacementRule() {
  return new InputRule(
    /(^|[\s([{])(:[\w+-]+:)$/,
    (state, match, start, end) => {
      if (isInCodeInputContext(state)) return null;
      const prefix = match[1] ?? "";
      const emoji = match[2] ? getEmojiByShortcode(match[2]) : undefined;
      if (!emoji) return null;
      return state.tr.insertText(emoji, start + prefix.length, end);
    },
  );
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
      emojiReplacementRule(),
      quoteRule(/(?:^|[\s{[(<'"‘“])(")$/, "“"),
      quoteRule(/"$/, "”"),
      quoteRule(/(?:^|[\s{[(<'"‘“])(')$/, "‘"),
      quoteRule(/'$/, "’"),
      textReplacementRule(/\.\.\.$/, "…"),
      markInputRule(/(^|[^*])\*\*([^*]+)\*\*$/, schema.marks.bold, 2),
      markInputRule(/(^|[^_])__([^_]+)__$/, schema.marks.bold, 2),
      markInputRule(/(^|[^~])~~([^~]+)~~$/, schema.marks.strike, 2),
      markInputRule(/(^|[^=])==([^=]+)==$/, schema.marks.highlight, 2),
      markInputRule(/(^|[^*])\*([^*]+)\*$/, schema.marks.italic, 1),
      markInputRule(/(^|[^_])_([^_]+)_$/, schema.marks.italic, 1),
      markInputRule(/(^|[^~])~([^~]+)~$/, schema.marks.strike, 1),
      markInputRule(/(^|[^`])`([^`]+)`$/, schema.marks.code, 1),
      underlineRule(),
    ],
  });
}

function findItemDepth($from: ResolvedPos, itemType: NodeType): number | null {
  for (let depth = $from.depth; depth > 0; depth--) {
    if ($from.node(depth).type === itemType) return depth;
  }
  return null;
}

function atStartOfItem($from: ResolvedPos, itemDepth: number): boolean {
  if ($from.parentOffset !== 0) return false;
  for (let depth = $from.depth; depth > itemDepth; depth--) {
    if ($from.index(depth - 1) !== 0) return false;
  }
  return true;
}

// Enter at the very start of the first list item inserts an empty paragraph
// before the list, giving the writer a way to open a row above it.
function insertParagraphBeforeFirstListItem(itemType: NodeType): Command {
  return (state, dispatch) => {
    if (!state.selection.empty) return false;

    const paragraphType = schema.nodes.paragraph;
    const { $from } = state.selection;
    const itemDepth = findItemDepth($from, itemType);
    if (itemDepth === null) return false;
    if (!atStartOfItem($from, itemDepth)) return false;
    if (
      $from.parent.type !== paragraphType ||
      $from.parent.content.size === 0
    ) {
      return false;
    }

    const listDepth = itemDepth - 1;
    const listParentDepth = listDepth - 1;
    if (listParentDepth < 0) return false;
    if ($from.index(listDepth) !== 0) return false;

    // Inside a nested list the paragraph would land inside the parent item;
    // fall through to splitListItem so Enter makes an empty sibling instead.
    const listParentName = $from.node(listParentDepth).type.name;
    if (listParentName === "listItem" || listParentName === "taskItem") {
      return false;
    }

    const listIndex = $from.index(listParentDepth);
    if (
      !$from
        .node(listParentDepth)
        .canReplaceWith(listIndex, listIndex, paragraphType)
    ) {
      return false;
    }

    const paragraph = paragraphType.createAndFill();
    if (!paragraph) return false;

    if (dispatch) {
      const insertPos = $from.before(listDepth);
      const tr = state.tr.insert(insertPos, paragraph);
      tr.setSelection(TextSelection.create(tr.doc, insertPos + 1));
      dispatch(tr.scrollIntoView());
    }
    return true;
  };
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
    insertParagraphBeforeFirstListItem(schema.nodes.taskItem),
    insertParagraphBeforeFirstListItem(schema.nodes.listItem),
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
      // New rows get fresh task identity and never inherit the done state — a
      // copied done status would mark an untouched row complete.
      const itemAttrs =
        itemName === "taskItem" ? createTaskItemAttrs() : undefined;
      return splitListItem(nodeType, itemAttrs)(state, dispatch);
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

  const backspaceCommands: Command[] = [
    (state, _dispatch) => {
      const { selection } = state;
      if (selection.$head.pos === 0 && selection.empty) return true;
      return false;
    },
    revertBlockToParagraph,
    joinTaskItemBackward,
    joinBackward,
    selectNodeBackward,
  ];
  // Only a plain Backspace should roll back a just-fired input rule;
  // modified deletion chords (Mod/Shift/Alt-Backspace) always delete.
  const backspaceCmd: Command = chainCommands(
    deleteSelection,
    undoInputRule,
    ...backspaceCommands,
  );
  const modifiedBackspaceCmd: Command = chainCommands(
    deleteSelection,
    ...backspaceCommands,
  );
  keys["Backspace"] = backspaceCmd;
  keys["Mod-Backspace"] = modifiedBackspaceCmd;
  keys["Shift-Backspace"] = modifiedBackspaceCmd;

  const deleteCmd: Command = chainCommands(
    deleteSelection,
    joinForward,
    selectNodeForward,
  );
  keys["Delete"] = deleteCmd;
  keys["Mod-Delete"] = deleteCmd;

  keys["Mod-a"] = selectAll;

  if (mac) {
    keys["Ctrl-h"] = modifiedBackspaceCmd;
    keys["Alt-Backspace"] = modifiedBackspaceCmd;
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
