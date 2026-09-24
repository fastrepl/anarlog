import {
  EditorState,
  Selection,
  TextSelection,
  type Transaction,
} from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { describe, expect, it } from "vitest";

import { buildInputRules, buildKeymap } from "./keymap";
import { schema } from "./schema";

describe("buildInputRules", () => {
  it("creates an unchecked task item when typing [] followed by space", () => {
    const inputRules = buildInputRules();
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("[]")]),
    ]);
    let state = EditorState.create({
      schema,
      doc,
      selection: Selection.atEnd(doc),
      plugins: [inputRules],
    });

    const view = {
      composing: false,
      get state() {
        return state;
      },
      dispatch(tr: Transaction) {
        state = state.apply(tr);
      },
    } as Pick<EditorView, "composing" | "dispatch" | "state"> as EditorView;

    const handleTextInput = inputRules.props.handleTextInput as
      | ((
          view: EditorView,
          from: number,
          to: number,
          text: string,
          deflt: () => Transaction,
        ) => boolean | void)
      | undefined;

    const handled = handleTextInput?.(
      view,
      state.selection.from,
      state.selection.to,
      " ",
      () => state.tr.insertText(" ", state.selection.from, state.selection.to),
    );

    expect(handled).toBe(true);
    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: {
                status: "todo",
                checked: false,
                taskId: expect.any(String),
                taskItemId: expect.any(String),
              },
              content: [{ type: "paragraph" }],
            },
          ],
        },
      ],
    });
  });

  it("keeps the cursor inside the new task before another paragraph", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("[]")]),
      schema.node("paragraph"),
    ]);
    const { state } = runTextInput(doc, " ", 3);

    expect(state.selection.$from.parent.type.name).toBe("paragraph");
    expect(state.selection.$from.node(-1).type.name).toBe("taskItem");
    expect(state.tr.insertText("My task").doc.firstChild?.textContent).toBe(
      "My task",
    );
  });

  it("creates a checked task item when typing [x] or [X] followed by space", () => {
    for (const marker of ["[x]", "[X]"]) {
      const doc = schema.node("doc", null, [
        schema.node("paragraph", null, [schema.text(marker)]),
      ]);
      const { handled, state } = runTextInput(doc, " ");

      expect(handled).toBe(true);
      expect(state.doc.toJSON()).toEqual({
        type: "doc",
        content: [
          {
            type: "taskList",
            content: [
              {
                type: "taskItem",
                attrs: {
                  status: "done",
                  checked: true,
                  taskId: expect.any(String),
                  taskItemId: expect.any(String),
                },
                content: [{ type: "paragraph" }],
              },
            ],
          },
        ],
      });
    }
  });

  it("keeps text typed after the marker inside the new task item", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("[]rest")]),
    ]);
    const { handled, state } = runTextInput(doc, " ", 3);

    expect(handled).toBe(true);
    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: {
                status: "todo",
                checked: false,
                taskId: expect.any(String),
                taskItemId: expect.any(String),
              },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "rest" }],
                },
              ],
            },
          ],
        },
      ],
    });
  });

  it("converts a bullet list item into a task item and keeps the rest of the list", () => {
    const doc = schema.node("doc", null, [
      schema.node("bulletList", null, [
        schema.node("listItem", null, [
          schema.node("paragraph", null, [schema.text("one")]),
        ]),
        schema.node("listItem", null, [
          schema.node("paragraph", null, [schema.text("[]")]),
        ]),
      ]),
    ]);
    const { handled, state } = runTextInput(doc, " ");

    expect(handled).toBe(true);
    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "bulletList",
          content: [
            {
              type: "listItem",
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "one" }],
                },
              ],
            },
          ],
        },
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: {
                status: "todo",
                checked: false,
                taskId: expect.any(String),
                taskItemId: expect.any(String),
              },
              content: [{ type: "paragraph" }],
            },
          ],
        },
      ],
    });
    expect(state.selection.$from.parent.type.name).toBe("paragraph");
    expect(state.selection.$from.node(-1).type.name).toBe("taskItem");
  });

  it("converts an ordered list item into a task item", () => {
    const doc = schema.node("doc", null, [
      schema.node("orderedList", { start: 5 }, [
        schema.node("listItem", null, [
          schema.node("paragraph", null, [schema.text("one")]),
        ]),
        schema.node("listItem", null, [
          schema.node("paragraph", null, [schema.text("[]")]),
        ]),
        schema.node("listItem", null, [
          schema.node("paragraph", null, [schema.text("two")]),
        ]),
      ]),
    ]);
    const { handled, state } = runTextInput(
      doc,
      " ",
      getTextStartPos(doc, "[]") + 2,
    );

    expect(handled).toBe(true);
    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "orderedList",
          attrs: { start: 5 },
          content: [
            {
              type: "listItem",
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "one" }],
                },
              ],
            },
          ],
        },
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: {
                status: "todo",
                checked: false,
                taskId: expect.any(String),
                taskItemId: expect.any(String),
              },
              content: [{ type: "paragraph" }],
            },
          ],
        },
        {
          type: "orderedList",
          attrs: { start: 7 },
          content: [
            {
              type: "listItem",
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
    });
  });

  it("merges a converted list item into adjacent task lists", () => {
    const doc = schema.node("doc", null, [
      schema.node("taskList", null, [
        schema.node(
          "taskItem",
          {
            status: "todo",
            checked: false,
            taskId: "task-1",
            taskItemId: "task-item-1",
          },
          [schema.node("paragraph", null, [schema.text("first")])],
        ),
      ]),
      schema.node("bulletList", null, [
        schema.node("listItem", null, [
          schema.node("paragraph", null, [schema.text("[]")]),
        ]),
      ]),
      schema.node("taskList", null, [
        schema.node(
          "taskItem",
          {
            status: "done",
            checked: true,
            taskId: "task-2",
            taskItemId: "task-item-2",
          },
          [schema.node("paragraph", null, [schema.text("last")])],
        ),
      ]),
    ]);
    const { handled, state } = runTextInput(
      doc,
      " ",
      getTextStartPos(doc, "[]") + 2,
    );

    expect(handled).toBe(true);
    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: {
                status: "todo",
                checked: false,
                taskId: "task-1",
                taskItemId: "task-item-1",
              },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "first" }],
                },
              ],
            },
            {
              type: "taskItem",
              attrs: {
                status: "todo",
                checked: false,
                taskId: expect.any(String),
                taskItemId: expect.any(String),
              },
              content: [{ type: "paragraph" }],
            },
            {
              type: "taskItem",
              attrs: {
                status: "done",
                checked: true,
                taskId: "task-2",
                taskItemId: "task-item-2",
              },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "last" }],
                },
              ],
            },
          ],
        },
      ],
    });
  });

  it("leaves a marker in a non-leading list item paragraph as literal text", () => {
    const doc = schema.node("doc", null, [
      schema.node("bulletList", null, [
        schema.node("listItem", null, [
          schema.node("paragraph", null, [schema.text("one")]),
          schema.node("paragraph", null, [schema.text("[]")]),
        ]),
      ]),
    ]);
    const { handled, state } = runTextInput(doc, " ");

    expect(handled).not.toBe(true);
    expect(state.doc.toJSON()).toEqual(doc.toJSON());
  });

  it("marks text between single backticks as code", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("use `code")]),
    ]);
    const { handled, state } = runTextInput(doc, "`");

    expect(handled).toBe(true);
    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            { type: "text", text: "use " },
            {
              type: "text",
              text: "code",
              marks: [{ type: "code" }],
            },
          ],
        },
      ],
    });
  });

  it("marks text between double underscores as bold", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("__bold_")]),
    ]);
    const { handled, state } = runTextInput(doc, "_");

    expect(handled).toBe(true);
    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "text",
              text: "bold",
              marks: [{ type: "bold" }],
            },
          ],
        },
      ],
    });
  });

  it("marks <u>wrapped</u> text as underline", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("<u>hi</u")]),
    ]);
    const { handled, state } = runTextInput(doc, ">");

    expect(handled).toBe(true);
    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "text",
              text: "hi",
              marks: [{ type: "underline" }],
            },
          ],
        },
      ],
    });
  });

  it("leaves <u>markup</u> literal inside a code span", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [
        schema.text("<u>hi</u", [schema.marks.code.create()]),
      ]),
    ]);
    const { handled, state } = runTextInput(doc, ">");

    expect(handled).not.toBe(true);
    expect(state.doc.toJSON()).toEqual(doc.toJSON());
  });

  it("leaves bold markup literal inside a code span", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [
        schema.text("**hi*", [schema.marks.code.create()]),
      ]),
    ]);
    const { handled, state } = runTextInput(doc, "*");

    expect(handled).not.toBe(true);
    expect(state.doc.toJSON()).toEqual(doc.toJSON());
  });

  it("replaces a known emoji shortcode with the emoji", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("launch :rocket")]),
    ]);
    const { handled, state } = runTextInput(doc, ":");

    expect(handled).toBe(true);
    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "launch 🚀" }],
        },
      ],
    });
  });

  it("leaves an unknown emoji shortcode as literal text", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("a :nosuchshortcode")]),
    ]);
    const { handled, state } = runTextInput(doc, ":");

    expect(handled).not.toBe(true);
    expect(state.doc.toJSON()).toEqual(doc.toJSON());
  });

  it("replaces typed arrow shorthand with an arrow symbol", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("-")]),
    ]);
    const { handled, state } = runTextInput(doc, ">");

    expect(handled).toBe(true);
    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "→" }],
        },
      ],
    });
  });

  it("replaces a double dash after a word with an em dash", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("wait-")]),
    ]);
    const { handled, state } = runTextInput(doc, "-");

    expect(handled).toBe(true);
    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "wait—" }],
        },
      ],
    });
  });

  it("leaves a third dash alone so --- can still become a horizontal rule", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("--")]),
    ]);
    const { handled, state } = runTextInput(doc, "-");

    expect(handled).toBeFalsy();
    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "--" }],
        },
      ],
    });
  });

  it("turns --- followed by a space into a horizontal rule", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("---")]),
    ]);
    const { handled, state } = runTextInput(doc, " ");

    expect(handled).toBe(true);
    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [{ type: "horizontalRule" }, { type: "paragraph" }],
    });
  });

  it("replaces typed copyright shorthand with a copyright symbol", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("(c")]),
    ]);
    const { handled, state } = runTextInput(doc, ")");

    expect(handled).toBe(true);
    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "©" }],
        },
      ],
    });
  });

  it("keeps replacement shorthands literal in code blocks", () => {
    const doc = schema.node("doc", null, [
      schema.node("codeBlock", null, [schema.text("-")]),
    ]);
    const { handled, state } = runTextInput(doc, ">");

    expect(handled).not.toBe(true);
    expect(state.doc.toJSON()).toEqual(doc.toJSON());
  });
});

describe("buildKeymap", () => {
  it("does not handle Shift+Enter as a hard break shortcut", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("hello")]),
    ]);
    const { handled, state } = runKeyDownAtEnd(doc, "Enter", {
      shiftKey: true,
    });

    expect(handled).not.toBe(true);
    expect(state.doc.toJSON()).toEqual(doc.toJSON());
  });

  it("merges task item text backward without changing the list structure", () => {
    const doc = schema.node("doc", null, [
      schema.node("taskList", null, [
        schema.node(
          "taskItem",
          {
            status: "todo",
            checked: false,
            taskId: "task-1",
            taskItemId: "task-item-1",
          },
          [schema.node("paragraph", null, [schema.text("one")])],
        ),
        schema.node(
          "taskItem",
          {
            status: "todo",
            checked: false,
            taskId: "task-2",
            taskItemId: "task-item-2",
          },
          [schema.node("paragraph", null, [schema.text("two")])],
        ),
      ]),
    ]);
    const { state } = runBackspaceAtTextStart(doc, "two");

    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: {
                status: "todo",
                checked: false,
                taskId: "task-1",
                taskItemId: "task-item-1",
              },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "onetwo" }],
                },
              ],
            },
          ],
        },
      ],
    });
  });

  it("lifts the first task item to a paragraph instead of merging into a previous bullet list", () => {
    const doc = schema.node("doc", null, [
      schema.node("bulletList", null, [
        schema.node("listItem", null, [
          schema.node("paragraph", null, [schema.text("one")]),
        ]),
      ]),
      schema.node("taskList", null, [
        schema.node(
          "taskItem",
          {
            status: "todo",
            checked: false,
            taskId: "task-1",
            taskItemId: "task-item-1",
          },
          [schema.node("paragraph", null, [schema.text("two")])],
        ),
      ]),
    ]);
    const { state } = runBackspaceAtTextStart(doc, "two");

    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "bulletList",
          content: [
            {
              type: "listItem",
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "one" }],
                },
              ],
            },
          ],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "two" }],
        },
      ],
    });
  });

  it("lifts the first task item to a paragraph when the list starts the doc", () => {
    const doc = schema.node("doc", null, [
      schema.node("taskList", null, [
        schema.node(
          "taskItem",
          {
            status: "todo",
            checked: false,
            taskId: "task-1",
            taskItemId: "task-item-1",
          },
          [schema.node("paragraph", null, [schema.text("two")])],
        ),
        schema.node(
          "taskItem",
          {
            status: "todo",
            checked: false,
            taskId: "task-2",
            taskItemId: "task-item-2",
          },
          [schema.node("paragraph", null, [schema.text("three")])],
        ),
      ]),
    ]);
    const { state } = runBackspaceAtTextStart(doc, "two");

    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "two" }],
        },
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: {
                status: "todo",
                checked: false,
                taskId: "task-2",
                taskItemId: "task-item-2",
              },
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

  it("splits a done task item into a fresh todo item on Enter", () => {
    const doc = schema.node("doc", null, [
      schema.node("taskList", null, [
        schema.node(
          "taskItem",
          {
            status: "done",
            checked: true,
            taskId: "task-1",
            taskItemId: "task-item-1",
          },
          [schema.node("paragraph", null, [schema.text("done")])],
        ),
      ]),
    ]);
    const { handled, state } = runKeyDownAtPos(
      doc,
      getTextStartPos(doc, "done") + "done".length,
      "Enter",
    );

    expect(handled).toBe(true);
    const json = state.doc.toJSON();
    expect(json.content?.[0]?.type).toBe("taskList");
    const items = json.content?.[0]?.content ?? [];
    expect(items).toHaveLength(2);
    expect(items[0]?.attrs).toMatchObject({
      status: "done",
      checked: true,
      taskId: "task-1",
      taskItemId: "task-item-1",
    });
    expect(items[1]?.attrs).toMatchObject({ status: "todo", checked: false });
    expect(items[1]?.attrs?.taskId).not.toBe("task-1");
    expect(items[1]?.attrs?.taskItemId).not.toBe("task-item-1");
    expect(state.selection.$from.node(-1).type.name).toBe("taskItem");
  });

  it("inserts an empty paragraph above the list on Enter at the start of the first task item", () => {
    const doc = schema.node("doc", null, [
      schema.node("taskList", null, [
        schema.node(
          "taskItem",
          {
            status: "todo",
            checked: false,
            taskId: "task-1",
            taskItemId: "task-item-1",
          },
          [schema.node("paragraph", null, [schema.text("task")])],
        ),
      ]),
    ]);
    const { handled, state } = runKeyDownAtPos(
      doc,
      getTextStartPos(doc, "task"),
      "Enter",
    );

    expect(handled).toBe(true);
    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        { type: "paragraph" },
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: {
                status: "todo",
                checked: false,
                taskId: "task-1",
                taskItemId: "task-item-1",
              },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "task" }],
                },
              ],
            },
          ],
        },
      ],
    });
    expect(state.selection.$from.parent.type.name).toBe("paragraph");
    expect(state.selection.$from.depth).toBe(1);
  });

  it("inserts an empty paragraph above the list on Enter at the start of the first bullet item", () => {
    const doc = schema.node("doc", null, [
      schema.node("bulletList", null, [
        schema.node("listItem", null, [
          schema.node("paragraph", null, [schema.text("one")]),
        ]),
        schema.node("listItem", null, [
          schema.node("paragraph", null, [schema.text("two")]),
        ]),
      ]),
    ]);
    const { handled, state } = runKeyDownAtPos(
      doc,
      getTextStartPos(doc, "one"),
      "Enter",
    );

    expect(handled).toBe(true);
    const json = state.doc.toJSON();
    expect(json.content?.[0]?.type).toBe("paragraph");
    expect(json.content?.[1]?.type).toBe("bulletList");
    expect(json.content?.[1]?.content).toHaveLength(2);
  });

  it("splits a bullet item mid-list on Enter instead of opening a paragraph above", () => {
    const doc = schema.node("doc", null, [
      schema.node("bulletList", null, [
        schema.node("listItem", null, [
          schema.node("paragraph", null, [schema.text("one")]),
        ]),
        schema.node("listItem", null, [
          schema.node("paragraph", null, [schema.text("two")]),
        ]),
      ]),
    ]);
    const { handled, state } = runKeyDownAtPos(
      doc,
      getTextStartPos(doc, "two"),
      "Enter",
    );

    expect(handled).toBe(true);
    const json = state.doc.toJSON();
    expect(json.content).toHaveLength(1);
    expect(json.content?.[0]?.type).toBe("bulletList");
    expect(json.content?.[0]?.content).toHaveLength(3);
  });

  it("restores the marker text when Backspace follows the task conversion", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("[]")]),
    ]);
    const { state } = runInputThenBackspace(doc, " ");

    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "[] " }],
        },
      ],
    });
  });

  it("does not undo the input rule on modified Backspace chords", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("a -")]),
    ]);
    const { handled, state } = runInputThenBackspace(
      doc,
      ">",
      undefined,
      "Backspace",
      { ctrlKey: true },
    );

    // A modified Backspace deletes backward instead of restoring the "-";
    // nothing in the chain claimed it, so the arrow stays for the browser's
    // own word-deletion to remove.
    expect(handled).toBeFalsy();
    expect(state.doc.firstChild?.textContent).toBe("a →");
  });

  it("joins later task item paragraphs within the same task item", () => {
    const doc = schema.node("doc", null, [
      schema.node("taskList", null, [
        schema.node(
          "taskItem",
          {
            status: "todo",
            checked: false,
            taskId: "task-1",
            taskItemId: "task-item-1",
          },
          [schema.node("paragraph", null, [schema.text("one")])],
        ),
        schema.node(
          "taskItem",
          {
            status: "todo",
            checked: false,
            taskId: "task-2",
            taskItemId: "task-item-2",
          },
          [
            schema.node("paragraph", null, [schema.text("two")]),
            schema.node("paragraph", null, [schema.text("three")]),
          ],
        ),
      ]),
    ]);
    const { state } = runBackspaceAtTextStart(doc, "three", true);

    expect(state.doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: {
                status: "todo",
                checked: false,
                taskId: "task-1",
                taskItemId: "task-item-1",
              },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "one" }],
                },
              ],
            },
            {
              type: "taskItem",
              attrs: {
                status: "todo",
                checked: false,
                taskId: "task-2",
                taskItemId: "task-item-2",
              },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "twothree" }],
                },
              ],
            },
          ],
        },
      ],
    });
  });
});

function runKeyDownAtPos(
  doc: ReturnType<typeof schema.node>,
  pos: number,
  key: string,
  init?: KeyboardEventInit,
) {
  const keymap = buildKeymap();
  let state = EditorState.create({
    schema,
    doc,
    selection: TextSelection.create(doc, pos),
    plugins: [keymap],
  });
  const view = {
    get state() {
      return state;
    },
    dispatch(tr: Transaction) {
      state = state.apply(tr);
    },
    endOfTextblock: () => false,
  } as Pick<EditorView, "dispatch" | "endOfTextblock" | "state"> as EditorView;
  const handleKeyDown = keymap.props.handleKeyDown;

  const handled = handleKeyDown?.(
    view,
    new KeyboardEvent("keydown", {
      key,
      ...init,
    }),
  );

  return { handled, state };
}

function runKeyDownAtEnd(
  doc: ReturnType<typeof schema.node>,
  key: string,
  init?: KeyboardEventInit,
) {
  const keymap = buildKeymap();
  let state = EditorState.create({
    schema,
    doc,
    selection: Selection.atEnd(doc),
    plugins: [keymap],
  });
  const view = {
    get state() {
      return state;
    },
    dispatch(tr: Transaction) {
      state = state.apply(tr);
    },
    endOfTextblock: () => false,
  } as Pick<EditorView, "dispatch" | "endOfTextblock" | "state"> as EditorView;
  const handleKeyDown = keymap.props.handleKeyDown;

  const handled = handleKeyDown?.(
    view,
    new KeyboardEvent("keydown", {
      key,
      ...init,
    }),
  );

  return { handled, state };
}

function runBackspaceAtTextStart(
  doc: ReturnType<typeof schema.node>,
  text: string,
  isEndOfTextblock = false,
) {
  const keymap = buildKeymap();
  const textPos = getTextStartPos(doc, text);
  let state = EditorState.create({
    schema,
    doc,
    selection: TextSelection.create(doc, textPos),
    plugins: [keymap],
  });
  const view = {
    get state() {
      return state;
    },
    dispatch(tr: Transaction) {
      state = state.apply(tr);
    },
    endOfTextblock: () => isEndOfTextblock,
  } as Pick<EditorView, "dispatch" | "endOfTextblock" | "state"> as EditorView;
  const handleKeyDown = keymap.props.handleKeyDown;

  const handled = handleKeyDown?.(
    view,
    new KeyboardEvent("keydown", { key: "Backspace" }),
  );

  expect(handled).toBe(true);
  return { state };
}

function runTextInput(
  doc: ReturnType<typeof schema.node>,
  text: string,
  pos?: number,
) {
  const inputRules = buildInputRules();
  let state = EditorState.create({
    schema,
    doc,
    selection:
      pos === undefined ? Selection.atEnd(doc) : TextSelection.create(doc, pos),
    plugins: [inputRules],
  });

  const view = {
    composing: false,
    get state() {
      return state;
    },
    dispatch(tr: Transaction) {
      state = state.apply(tr);
    },
  } as Pick<EditorView, "composing" | "dispatch" | "state"> as EditorView;

  const handleTextInput = inputRules.props.handleTextInput as
    | ((
        view: EditorView,
        from: number,
        to: number,
        text: string,
        deflt: () => Transaction,
      ) => boolean | void)
    | undefined;

  const handled = handleTextInput?.(
    view,
    state.selection.from,
    state.selection.to,
    text,
    () => state.tr.insertText(text, state.selection.from, state.selection.to),
  );

  return { handled, state };
}

function runInputThenBackspace(
  doc: ReturnType<typeof schema.node>,
  text: string,
  pos?: number,
  key: string = "Backspace",
  init?: KeyboardEventInit,
) {
  const inputRules = buildInputRules();
  const keymap = buildKeymap();
  let state = EditorState.create({
    schema,
    doc,
    selection:
      pos === undefined ? Selection.atEnd(doc) : TextSelection.create(doc, pos),
    plugins: [inputRules, keymap],
  });

  const view = {
    composing: false,
    get state() {
      return state;
    },
    dispatch(tr: Transaction) {
      state = state.apply(tr);
    },
    endOfTextblock: () => false,
  } as Pick<
    EditorView,
    "composing" | "dispatch" | "endOfTextblock" | "state"
  > as EditorView;

  const handleTextInput = inputRules.props.handleTextInput as
    | ((
        view: EditorView,
        from: number,
        to: number,
        text: string,
        deflt: () => Transaction,
      ) => boolean | void)
    | undefined;
  handleTextInput?.(view, state.selection.from, state.selection.to, text, () =>
    state.tr.insertText(text, state.selection.from, state.selection.to),
  );

  const handled = keymap.props.handleKeyDown?.(
    view,
    new KeyboardEvent("keydown", { key, ...init }),
  );

  return { handled, state };
}

function getTextStartPos(doc: ReturnType<typeof schema.node>, text: string) {
  let textPos = -1;

  doc.descendants((node, pos) => {
    if (node.isText && node.text === text) {
      textPos = pos;
      return false;
    }

    return undefined;
  });

  if (textPos === -1) {
    throw new Error(`Missing text node: ${text}`);
  }

  return textPos;
}
