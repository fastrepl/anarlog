import { GapCursor } from "prosemirror-gapcursor";
import { EditorState, TextSelection } from "prosemirror-state";
import { describe, expect, it } from "vitest";

import { schema } from "../note/schema";
import { placeholderPlugin } from "./placeholder";

describe("placeholderPlugin", () => {
  it("decorates only the selected empty top-level block", () => {
    const plugin = placeholderPlugin(
      ({ node }) => `${node.type.name} placeholder`,
    );
    let state = EditorState.create({
      schema,
      doc: schema.node("doc", null, [
        schema.node("paragraph", null, [schema.text("filled")]),
        schema.node("paragraph"),
      ]),
      plugins: [plugin],
    });

    expect(plugin.props.decorations?.(state)?.find()).toHaveLength(0);

    const emptyParagraphPos = state.doc.firstChild!.nodeSize;
    state = state.apply(
      state.tr.setSelection(
        TextSelection.create(state.doc, emptyParagraphPos + 1),
      ),
    );

    const decorations = plugin.props.decorations?.(state)?.find() ?? [];
    expect(decorations).toHaveLength(1);
    expect(decorations[0].from).toBe(emptyParagraphPos);
    expect(decorations[0].to).toBe(emptyParagraphPos + 2);
  });

  it("does not decorate a sibling empty block when selection is in filled text", () => {
    const plugin = placeholderPlugin(() => "Memo");
    const doc = schema.node("doc", null, [
      schema.node("paragraph"),
      schema.node("paragraph", null, [schema.text("filled")]),
    ]);
    const filledParagraphPos = doc.firstChild!.nodeSize;
    const state = EditorState.create({
      schema,
      doc,
      plugins: [plugin],
    }).apply(
      EditorState.create({ schema, doc }).tr.setSelection(
        TextSelection.create(doc, filledParagraphPos + 2),
      ),
    );

    expect(plugin.props.decorations?.(state)?.find()).toHaveLength(0);
  });

  it("adds the editor-empty class for an empty document", () => {
    const plugin = placeholderPlugin(() => "Memo");
    const state = EditorState.create({
      schema,
      doc: schema.node("doc", null, [schema.node("paragraph")]),
      plugins: [plugin],
    });

    const decorations = plugin.props.decorations?.(state)?.find() ?? [];
    expect(decorations).toHaveLength(1);
    expect(decorations[0].type.attrs.class).toContain("is-editor-empty");
  });

  it("decorates an empty block next to a gap cursor", () => {
    const plugin = placeholderPlugin(() => "Memo");
    const doc = schema.node("doc", null, [schema.node("paragraph")]);
    const state = EditorState.create({
      schema,
      doc,
      plugins: [plugin],
    });
    const gapCursorState = state.apply(
      state.tr.setSelection(new GapCursor(doc.resolve(0))),
    );

    const decorations =
      plugin.props.decorations?.(gapCursorState)?.find() ?? [];
    expect(decorations).toHaveLength(1);
    expect(decorations[0].from).toBe(0);
  });

  it("decorates the selected nested empty block", () => {
    const plugin = placeholderPlugin(() => "Nested");
    const doc = schema.node("doc", null, [
      schema.node("taskList", null, [
        schema.node(
          "taskItem",
          {
            checked: false,
            taskItemId: "task-item-1",
            taskId: "task-1",
            source: null,
            status: "todo",
          },
          [schema.node("paragraph")],
        ),
      ]),
    ]);
    const state = EditorState.create({ schema, doc, plugins: [plugin] });
    let emptyParagraphPos = 0;
    doc.descendants((node, pos) => {
      if (node.type === schema.nodes.paragraph) {
        emptyParagraphPos = pos;
        return false;
      }
    });
    const nestedState = state.apply(
      state.tr.setSelection(TextSelection.create(doc, emptyParagraphPos + 1)),
    );

    const decorations = plugin.props.decorations?.(nestedState)?.find() ?? [];
    expect(decorations).toHaveLength(1);
    expect(decorations[0].from).toBe(emptyParagraphPos);
  });
});
