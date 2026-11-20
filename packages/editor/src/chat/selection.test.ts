// @vitest-environment jsdom

import { history } from "prosemirror-history";
import type { Node as PMNode } from "prosemirror-model";
import { EditorState, TextSelection } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { afterEach, describe, expect, it } from "vitest";

import { chatSchema } from "./schema";
import { readChatSelection } from "./selection";

const views: EditorView[] = [];

afterEach(() => {
  for (const view of views.splice(0)) if (!view.isDestroyed) view.destroy();
});

function paragraph(...content: (string | PMNode)[]) {
  return chatSchema.node(
    "paragraph",
    null,
    content.map((part) =>
      typeof part === "string" ? chatSchema.text(part) : part,
    ),
  );
}

function editor(paragraphs: PMNode[], from: number, to = from) {
  const doc = chatSchema.node("doc", null, paragraphs);
  const view = new EditorView(document.createElement("div"), {
    state: EditorState.create({
      doc,
      selection: TextSelection.create(doc, from, to),
      plugins: [history()],
    }),
  });
  views.push(view);
  return { view, read: () => readChatSelection(view, () => view) };
}

function text(view: EditorView) {
  return view.state.doc.textBetween(0, view.state.doc.content.size, "\n");
}

describe("readChatSelection", () => {
  it("fits to the text before the caret and types over a joined period", () => {
    const { view, read } = editor(
      [paragraph("First line"), paragraph("We'll stay in.")],
      27,
    );
    const caret = read();
    expect(caret).toMatchObject({
      before: "First line\nWe'll stay in.",
      selected: null,
      empty: true,
    });

    expect(caret.replace(", if it snows.", { join: true })).toBe(true);
    expect(text(view)).toBe("First line\nWe'll stay in, if it snows.");

    expect(read().replace(" Then\nwe rest.")).toBe(true);
    expect(view.state.doc.childCount).toBe(3);
    expect(text(view)).toBe(
      "First line\nWe'll stay in, if it snows. Then\nwe rest.",
    );

    for (const [inserted, following, expected] of [
      ["Hello", "world", "Hello world"],
      ["Hello", ", world", "Hello, world"],
      ["Hello ", "world", "Hello world"],
      ["Hello", " world", "Hello world"],
      ["你好", "世界", "你好世界"],
      ["こんにちは", "世界", "こんにちは世界"],
    ]) {
      const middle = editor([paragraph(following)], 1);
      expect(
        middle.read().replace(inserted, { separateFollowingWord: true }),
      ).toBe(true);
      expect(text(middle.view)).toBe(expected);
    }
  });

  it("replaces only the selection that was read", () => {
    const { view, read } = editor([paragraph("Ship it Friday please")], 9, 15);
    const selection = read();
    expect(selection).toMatchObject({
      before: "Ship it ",
      selected: "Friday",
      empty: false,
    });

    expect(selection.replace("on Monday")).toBe(true);
    expect(text(view)).toBe("Ship it on Monday please");
  });

  it("writes nothing once the content, the selection or the editor changed", () => {
    const edited = editor([paragraph("Ship it Friday")], 9, 15);
    const beforeEdit = edited.read();
    edited.view.dispatch(edited.view.state.tr.insertText("Do ", 1));
    expect(beforeEdit.replace("Monday")).toBe(false);
    expect(text(edited.view)).toBe("Do Ship it Friday");

    const moved = editor([paragraph("Ship it Friday")], 9, 15);
    const beforeMove = moved.read();
    moved.view.dispatch(
      moved.view.state.tr.setSelection(
        TextSelection.create(moved.view.state.doc, 1, 5),
      ),
    );
    expect(beforeMove.replace("Monday")).toBe(false);
    expect(text(moved.view)).toBe("Ship it Friday");

    const replaced = editor([paragraph("Ship it Friday")], 9, 15);
    const other = editor([paragraph("Ship it Friday")], 9, 15);
    expect(
      readChatSelection(replaced.view, () => other.view).replace("Monday"),
    ).toBe(false);
    const beforeDestroy = replaced.read();
    replaced.view.destroy();
    expect(beforeDestroy.replace("Monday")).toBe(false);
  });

  it("offers no selection holding a mention for rewriting", () => {
    const mention = chatSchema.node("mention-@", {
      id: "p1",
      type: "human",
      label: "Mina",
    });
    const { read } = editor([paragraph("Ask ", mention, " today")], 1, 12);

    expect(read().selected).toBeNull();
    expect(
      editor([paragraph("Ask ", mention, " today")], 7).read().before,
    ).toBe("Ask @Mina ");
  });
});
