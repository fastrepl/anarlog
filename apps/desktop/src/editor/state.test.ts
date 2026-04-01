import { Plugin, PluginKey, TextSelection } from "prosemirror-state";
import { describe, expect, it } from "vitest";

import { schema } from "./session/schema";
import {
  createEditorState,
  parseEditorDoc,
  reconcileEditorState,
} from "./state";

function createEmptyDoc() {
  return schema.node("doc", null, [schema.node("paragraph")]);
}

function createPlugin(name: string) {
  return new Plugin({
    key: new PluginKey(name),
  });
}

describe("editor state helpers", () => {
  it("falls back to an empty doc for invalid content", () => {
    const doc = parseEditorDoc(
      schema,
      { type: "doc", content: [{ type: "missing-node" }] },
      createEmptyDoc,
    );

    expect(doc.toJSON()).toEqual(createEmptyDoc().toJSON());
  });

  it("reconfigures plugins without replacing the current selection", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("alpha")]),
    ]);
    const initialPlugins = [createPlugin("one")];
    const nextPlugins = [createPlugin("two")];
    const state = createStateWithSelection(doc, initialPlugins, 3);

    const nextState = reconcileEditorState({
      currentState: state,
      nextDoc: doc,
      plugins: nextPlugins,
      isViewFocused: true,
    });

    expect(nextState.doc).toBe(state.doc);
    expect(nextState.selection.from).toBe(state.selection.from);
    expect(nextState.plugins).toEqual(nextPlugins);
  });

  it("replaces the document when external content changes and the view is unfocused", () => {
    const currentDoc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("alpha")]),
    ]);
    const nextDoc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("beta")]),
    ]);
    const plugins = [createPlugin("shared")];
    const state = createStateWithSelection(currentDoc, plugins, 3);

    const nextState = reconcileEditorState({
      currentState: state,
      nextDoc,
      plugins,
      isViewFocused: false,
    });

    expect(nextState.doc.toJSON()).toEqual(nextDoc.toJSON());
    expect(nextState.plugins).toEqual(plugins);
    expect(nextState.selection.from).toBe(1);
  });

  it("keeps the current doc while focused but still reconfigures plugins", () => {
    const currentDoc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("alpha")]),
    ]);
    const nextDoc = schema.node("doc", null, [
      schema.node("paragraph", null, [schema.text("beta")]),
    ]);
    const initialPlugins = [createPlugin("one")];
    const nextPlugins = [createPlugin("two")];
    const state = createStateWithSelection(currentDoc, initialPlugins, 4);

    const nextState = reconcileEditorState({
      currentState: state,
      nextDoc,
      plugins: nextPlugins,
      isViewFocused: true,
    });

    expect(nextState.doc.toJSON()).toEqual(currentDoc.toJSON());
    expect(nextState.selection.from).toBe(4);
    expect(nextState.plugins).toEqual(nextPlugins);
  });
});

function createStateWithSelection(
  doc: ReturnType<typeof createEmptyDoc>,
  plugins: Plugin[],
  pos: number,
) {
  const state = createEditorState(doc, plugins);
  return state.apply(
    state.tr.setSelection(TextSelection.create(state.doc, pos)),
  );
}
