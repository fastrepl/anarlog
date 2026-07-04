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
    expect(state.doc.toJSON()).toMatchObject({
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

  it("keeps the first task item separate from a previous bullet list", () => {
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

    expect(state.doc.toJSON()).toEqual(doc.toJSON());
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

function runTextInput(doc: ReturnType<typeof schema.node>, text: string) {
  const inputRules = buildInputRules();
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
    text,
    () => state.tr.insertText(text, state.selection.from, state.selection.to),
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
