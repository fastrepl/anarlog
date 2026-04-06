import { Editor, type JSONContent } from "@tiptap/core";
import TaskItem from "@tiptap/extension-task-item";
import TaskList from "@tiptap/extension-task-list";
import { TextSelection } from "@tiptap/pm/state";
import StarterKit from "@tiptap/starter-kit";
import { afterEach, describe, expect, test } from "vitest";

import CustomListKeymap, {
  getSelectedListItemNames,
  isSelectionInListItem,
  liftSelectedListItems,
  sinkSelectedListItems,
} from "./custom-list-keymap";

const TASK_LIST_CONTENT: JSONContent = {
  type: "doc",
  content: [
    {
      type: "taskList",
      content: [
        {
          type: "taskItem",
          attrs: { checked: false },
          content: [
            {
              type: "paragraph",
              content: [{ type: "text", text: "one" }],
            },
          ],
        },
        {
          type: "taskItem",
          attrs: { checked: false },
          content: [
            {
              type: "paragraph",
              content: [{ type: "text", text: "two" }],
            },
          ],
        },
        {
          type: "taskItem",
          attrs: { checked: false },
          content: [
            {
              type: "paragraph",
              content: [{ type: "text", text: "three" }],
            },
          ],
        },
      ],
    },
  ],
};

const SPLIT_TASK_LIST_CONTENT: JSONContent = {
  type: "doc",
  content: [
    {
      type: "taskList",
      content: [
        {
          type: "taskItem",
          attrs: { checked: false },
          content: [
            {
              type: "paragraph",
              content: [{ type: "text", text: "one" }],
            },
          ],
        },
        {
          type: "taskItem",
          attrs: { checked: false },
          content: [
            {
              type: "paragraph",
              content: [{ type: "text", text: "two" }],
            },
          ],
        },
      ],
    },
    { type: "paragraph" },
    {
      type: "taskList",
      content: [
        {
          type: "taskItem",
          attrs: { checked: false },
          content: [
            {
              type: "paragraph",
              content: [{ type: "text", text: "three" }],
            },
          ],
        },
      ],
    },
  ],
};

const editors: Editor[] = [];

function createEditor(content = TASK_LIST_CONTENT): Editor {
  const editor = new Editor({
    extensions: [
      StarterKit.configure({ listKeymap: false }),
      TaskList,
      TaskItem.configure({ nested: true }),
      CustomListKeymap,
    ],
    content,
  });

  editors.push(editor);

  return editor;
}

function getTextPos(editor: Editor, text: string): number {
  let matchPos = -1;

  editor.state.doc.descendants((node, pos) => {
    if (node.isText && node.text === text) {
      matchPos = pos;
      return false;
    }

    return undefined;
  });

  if (matchPos === -1) {
    throw new Error(`Missing text node: ${text}`);
  }

  return matchPos + 1;
}

function setCursor(editor: Editor, text: string) {
  const pos = getTextPos(editor, text);

  editor.view.dispatch(
    editor.state.tr.setSelection(TextSelection.create(editor.state.doc, pos)),
  );
}

function setRange(editor: Editor, startText: string, endText: string) {
  const from = getTextPos(editor, startText);
  const to = getTextPos(editor, endText) + endText.length - 1;

  editor.view.dispatch(
    editor.state.tr.setSelection(
      TextSelection.create(editor.state.doc, from, to),
    ),
  );
}

afterEach(() => {
  while (editors.length > 0) {
    editors.pop()?.destroy();
  }
});

describe("custom list keymap", () => {
  test("detects task item selections for cursors and ranges", () => {
    const editor = createEditor();

    setCursor(editor, "two");
    expect(getSelectedListItemNames(editor.state)).toEqual(["taskItem"]);
    expect(isSelectionInListItem(editor.state)).toBe(true);

    setRange(editor, "two", "three");
    expect(getSelectedListItemNames(editor.state)).toEqual(["taskItem"]);
    expect(isSelectionInListItem(editor.state)).toBe(true);
  });

  test("sinks a single task item", () => {
    const editor = createEditor();

    setCursor(editor, "two");

    expect(sinkSelectedListItems(editor)).toBe(true);
    expect(editor.getJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: { checked: false },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "one" }],
                },
                {
                  type: "taskList",
                  content: [
                    {
                      type: "taskItem",
                      attrs: { checked: false },
                      content: [
                        {
                          type: "paragraph",
                          content: [{ type: "text", text: "two" }],
                        },
                      ],
                    },
                  ],
                },
              ],
            },
            {
              type: "taskItem",
              attrs: { checked: false },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "three" }],
                },
              ],
            },
          ],
        },
      ],
    });
  });

  test("sinks and lifts a ranged task item selection", () => {
    const editor = createEditor();

    setRange(editor, "two", "three");

    expect(sinkSelectedListItems(editor)).toBe(true);
    expect(editor.getJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: { checked: false },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "one" }],
                },
                {
                  type: "taskList",
                  content: [
                    {
                      type: "taskItem",
                      attrs: { checked: false },
                      content: [
                        {
                          type: "paragraph",
                          content: [{ type: "text", text: "two" }],
                        },
                      ],
                    },
                    {
                      type: "taskItem",
                      attrs: { checked: false },
                      content: [
                        {
                          type: "paragraph",
                          content: [{ type: "text", text: "three" }],
                        },
                      ],
                    },
                  ],
                },
              ],
            },
          ],
        },
      ],
    });

    expect(liftSelectedListItems(editor)).toBe(true);
    expect(editor.getJSON()).toEqual(TASK_LIST_CONTENT);
  });

  test("joins split task lists before indenting a ranged selection", () => {
    const editor = createEditor(SPLIT_TASK_LIST_CONTENT);

    setRange(editor, "two", "three");

    expect(sinkSelectedListItems(editor)).toBe(true);
    expect(editor.getJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: { checked: false },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "one" }],
                },
                {
                  type: "taskList",
                  content: [
                    {
                      type: "taskItem",
                      attrs: { checked: false },
                      content: [
                        {
                          type: "paragraph",
                          content: [{ type: "text", text: "two" }],
                        },
                      ],
                    },
                    {
                      type: "taskItem",
                      attrs: { checked: false },
                      content: [
                        {
                          type: "paragraph",
                          content: [{ type: "text", text: "three" }],
                        },
                      ],
                    },
                  ],
                },
              ],
            },
          ],
        },
      ],
    });
  });
});
