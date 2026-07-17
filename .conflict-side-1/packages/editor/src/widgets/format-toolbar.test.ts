import { EditorState, TextSelection } from "prosemirror-state";
import { describe, expect, it } from "vitest";

import { schema } from "../note/schema";
import { selectionTouchesTitleHeading } from "./format-toolbar";

function createState(from: number, to: number) {
  const doc = schema.node("doc", null, [
    schema.node("heading", { level: 1 }, [schema.text("Design sync")]),
    schema.node("paragraph", null, [schema.text("Follow up")]),
  ]);

  return EditorState.create({
    doc,
    selection: TextSelection.create(doc, from, to),
  });
}

describe("selectionTouchesTitleHeading", () => {
  it("returns true for selections inside the title heading", () => {
    expect(selectionTouchesTitleHeading(createState(1, 12))).toBe(true);
  });

  it("returns true for selections spanning the title heading", () => {
    expect(selectionTouchesTitleHeading(createState(6, 18))).toBe(true);
  });

  it("returns false for body selections", () => {
    expect(selectionTouchesTitleHeading(createState(14, 20))).toBe(false);
  });

  it("returns false when the first block is not a title heading", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("Body")]),
    ]);
    const state = EditorState.create({
      doc,
      selection: TextSelection.create(doc, 1, 3),
    });

    expect(selectionTouchesTitleHeading(state)).toBe(false);
  });
});
