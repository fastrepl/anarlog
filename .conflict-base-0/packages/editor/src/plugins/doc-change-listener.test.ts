import { EditorState, type Plugin } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { describe, expect, it, vi } from "vitest";

import { schema } from "../note/schema";
import { docChangeListenerPlugin } from "./doc-change-listener";

function createState(text = "", plugins: Plugin[] = []) {
  return EditorState.create({
    schema,
    doc: schema.node("doc", null, [
      schema.node("paragraph", null, text ? [schema.text(text)] : undefined),
    ]),
    plugins,
  });
}

describe("docChangeListenerPlugin", () => {
  it("reports only document changes", () => {
    const onDocChanged = vi.fn();
    const plugin = docChangeListenerPlugin(onDocChanged);
    const previousState = createState("", [plugin]);
    const pluginView = plugin.spec.view?.({
      state: previousState,
    } as EditorView);

    const selectionOnlyState = previousState.apply(
      previousState.tr.setMeta("source", "test"),
    );
    const selectionOnlyView = { state: selectionOnlyState } as EditorView;
    pluginView?.update(selectionOnlyView, previousState);

    expect(onDocChanged).not.toHaveBeenCalled();

    const changedState = previousState.apply(previousState.tr.insertText("x"));
    pluginView?.update({ state: changedState } as EditorView, previousState);

    expect(onDocChanged).toHaveBeenCalledOnce();
    expect(onDocChanged).toHaveBeenCalledWith(changedState.doc);
  });

  it("ignores document replacement from controlled prop sync", () => {
    const onDocChanged = vi.fn();
    const plugin = docChangeListenerPlugin(onDocChanged);
    const previousState = createState("old", [plugin]);
    const pluginView = plugin.spec.view?.({
      state: previousState,
    } as EditorView);
    const syncedState = createState("new", [plugin]);

    pluginView?.update({ state: syncedState } as EditorView, previousState);

    expect(onDocChanged).not.toHaveBeenCalled();
  });
});
