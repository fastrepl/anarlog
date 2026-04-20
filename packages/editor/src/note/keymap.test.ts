import { EditorState, Selection, type Transaction } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { describe, expect, it } from "vitest";

import { buildInputRules } from "./keymap";
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
});
