import { EditorState } from "prosemirror-state";
import { describe, expect, it } from "vitest";

import { schema } from "../note/schema";
import { hashtagPlugin, hashtagPluginKey } from "./hashtag";

describe("hashtagPlugin", () => {
  it("updates decorations for the changed block", () => {
    let state = createState("#one #two");

    expect(getHashtagDecorationCount(state)).toBe(2);

    state = state.apply(state.tr.delete(1, 5));

    expect(getHashtagDecorationCount(state)).toBe(1);
  });

  it("keeps decorations in unchanged blocks", () => {
    let state = EditorState.create({
      schema,
      doc: schema.node("doc", null, [
        schema.node("paragraph", null, [schema.text("#one")]),
        schema.node("paragraph", null, [schema.text("#two")]),
      ]),
      plugins: [hashtagPlugin()],
    });

    expect(getHashtagDecorationCount(state)).toBe(2);

    state = state.apply(state.tr.insertText("!", state.doc.content.size - 1));

    expect(getHashtagDecorationCount(state)).toBe(2);
  });
});

function createState(text: string) {
  return EditorState.create({
    schema,
    doc: schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text(text)]),
    ]),
    plugins: [hashtagPlugin()],
  });
}

function getHashtagDecorationCount(state: EditorState) {
  return hashtagPluginKey.getState(state).find().length;
}
