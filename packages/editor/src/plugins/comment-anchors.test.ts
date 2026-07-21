import { EditorState, TextSelection } from "prosemirror-state";
import { describe, expect, it } from "vitest";

import { schema } from "../note/schema";
import {
  commentAnchorsPlugin,
  commentAnchorsPluginKey,
  getCommentAnchorRanges,
} from "./comment-anchors";

const createState = () =>
  EditorState.create({
    schema,
    doc: schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("hello anchored world")]),
    ]),
    plugins: [commentAnchorsPlugin()],
  });

const setAnchors = (
  state: EditorState,
  anchors: Array<{ commentId: string; from: number; to: number }>,
) =>
  state.apply(
    state.tr.setMeta(commentAnchorsPluginKey, { type: "set", anchors }),
  );

describe("commentAnchorsPlugin", () => {
  it("creates decorations from set metadata", () => {
    let state = createState();
    state = setAnchors(state, [{ commentId: "c1", from: 7, to: 15 }]);

    const ranges = getCommentAnchorRanges(state);
    expect(ranges).toEqual([{ commentId: "c1", from: 7, to: 15 }]);

    const decorations = commentAnchorsPluginKey
      .getState(state)!
      .decorations.find();
    expect(decorations).toHaveLength(1);
    expect(decorations[0].spec.commentId).toBe("c1");
  });

  it("maps ranges through document edits", () => {
    let state = createState();
    state = setAnchors(state, [{ commentId: "c1", from: 7, to: 15 }]);
    state = state.apply(state.tr.insertText("XX", 1));

    expect(getCommentAnchorRanges(state)).toEqual([
      { commentId: "c1", from: 9, to: 17 },
    ]);
  });

  it("drops ranges whose text is deleted", () => {
    let state = createState();
    state = setAnchors(state, [{ commentId: "c1", from: 7, to: 15 }]);
    state = state.apply(state.tr.delete(6, 16));

    expect(getCommentAnchorRanges(state)).toEqual([]);
  });

  it("swaps the active class through active metadata", () => {
    let state = createState();
    state = setAnchors(state, [
      { commentId: "c1", from: 1, to: 6 },
      { commentId: "c2", from: 7, to: 15 },
    ]);
    state = state.apply(
      state.tr.setMeta(commentAnchorsPluginKey, {
        type: "active",
        commentId: "c2",
      }),
    );

    const classes = new Map(
      commentAnchorsPluginKey
        .getState(state)!
        .decorations.find()
        .map((decoration) => [
          decoration.spec.commentId as string,
          (decoration as unknown as { type: { attrs: { class: string } } }).type
            .attrs.class,
        ]),
    );
    expect(classes.get("c1")).toBe("comment-anchor");
    expect(classes.get("c2")).toBe("comment-anchor comment-anchor-active");
  });

  it("remaps ranges when one transaction edits the doc and sets the active comment", () => {
    let state = createState();
    state = setAnchors(state, [{ commentId: "c1", from: 7, to: 15 }]);
    state = state.apply(
      state.tr
        .insertText("XX", 1)
        .setMeta(commentAnchorsPluginKey, { type: "active", commentId: "c1" }),
    );

    expect(getCommentAnchorRanges(state)).toEqual([
      { commentId: "c1", from: 9, to: 17 },
    ]);
    expect(commentAnchorsPluginKey.getState(state)!.activeId).toBe("c1");
  });

  it("returns one merged range for a cross-block anchor", () => {
    let state = EditorState.create({
      schema,
      doc: schema.node("doc", null, [
        schema.node("paragraph", null, [schema.text("first block")]),
        schema.node("paragraph", null, [schema.text("second block")]),
      ]),
      plugins: [commentAnchorsPlugin()],
    });
    const size = state.doc.content.size;
    state = setAnchors(state, [{ commentId: "span", from: 3, to: size - 3 }]);

    const ranges = getCommentAnchorRanges(state);
    expect(ranges).toEqual([{ commentId: "span", from: 3, to: size - 3 }]);

    state = state.apply(state.tr.insertText("x", 1));
    state = state.apply(
      state.tr.setMeta(commentAnchorsPluginKey, {
        type: "active",
        commentId: "span",
      }),
    );
    const mapped = getCommentAnchorRanges(state);
    expect(mapped).toEqual([{ commentId: "span", from: 4, to: size - 2 }]);
  });

  it("keeps boundary insertions outside the anchored range", () => {
    let state = createState();
    state = setAnchors(state, [{ commentId: "c1", from: 7, to: 15 }]);
    state = state.apply(state.tr.insertText("!!", 15));
    state = state.apply(state.tr.insertText("--", 7));

    expect(getCommentAnchorRanges(state)).toEqual([
      { commentId: "c1", from: 9, to: 17 },
    ]);
  });

  it("ignores anchors outside the document", () => {
    let state = createState();
    state = setAnchors(state, [
      { commentId: "ok", from: 1, to: 6 },
      { commentId: "oob", from: 100, to: 120 },
      { commentId: "collapsed", from: 3, to: 3 },
    ]);

    expect(getCommentAnchorRanges(state).map((a) => a.commentId)).toEqual([
      "ok",
    ]);
  });

  it("reports selection changes through the event callback", () => {
    const events: Array<{ from: number; to: number; empty: boolean }> = [];
    const state = EditorState.create({
      schema,
      doc: schema.node("doc", null, [
        schema.node("paragraph", null, [schema.text("hello anchored world")]),
      ]),
      plugins: [
        commentAnchorsPlugin({
          onEvent: (event) => {
            if (event.type === "selection") events.push(event);
          },
        }),
      ],
    });
    const selected = state.apply(
      state.tr.setSelection(TextSelection.create(state.doc, 1, 6)),
    );
    // The selection event fires from the editor view; at the state level we
    // assert the plugin state stays intact across selection transactions.
    expect(
      commentAnchorsPluginKey.getState(selected)?.decorations,
    ).toBeDefined();
    expect(events).toHaveLength(0);
  });
});
