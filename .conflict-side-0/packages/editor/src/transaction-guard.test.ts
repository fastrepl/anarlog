import { EditorState, type Transaction } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { afterAll, afterEach, describe, expect, it, vi } from "vitest";

import { schema } from "./note/schema";
import { dispatchEditorTransaction } from "./transaction-guard";

const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

afterEach(() => {
  consoleError.mockClear();
});

afterAll(() => {
  consoleError.mockRestore();
});

describe("dispatchEditorTransaction", () => {
  it("applies document changes and reports updates", () => {
    let state = EditorState.create({
      schema,
      doc: schema.node("doc", null, [schema.node("paragraph")]),
    });
    const view = {
      get state() {
        return state;
      },
      updateState(nextState: EditorState) {
        state = nextState;
      },
    } as Pick<EditorView, "state" | "updateState"> as EditorView;
    const onDocChanged = vi.fn();

    dispatchEditorTransaction({
      view,
      transaction: state.tr.insertText("hello", 1),
      onDocChanged,
    });

    expect(state.doc.textContent).toBe("hello");
    expect(onDocChanged).toHaveBeenCalledOnce();
  });

  it("catches stale transactions without throwing", () => {
    const initialState = EditorState.create({
      schema,
      doc: schema.node("doc", null, [schema.node("paragraph")]),
    });
    const staleTransaction = initialState.tr.insertText("stale", 1);
    let state = initialState.apply(initialState.tr.insertText("current", 1));
    const view = {
      get state() {
        return state;
      },
      updateState(nextState: EditorState) {
        state = nextState;
      },
    } as Pick<EditorView, "state" | "updateState"> as EditorView;
    const onError = vi.fn();

    expect(() =>
      dispatchEditorTransaction({
        view,
        transaction: staleTransaction,
        onError,
      }),
    ).not.toThrow();

    expect(state.doc.textContent).toBe("current");
    expect(onError).toHaveBeenCalledWith(
      expect.any(RangeError),
      view,
      staleTransaction,
    );
  });

  it("does not report non-document transactions as content changes", () => {
    let state = EditorState.create({
      schema,
      doc: schema.node("doc", null, [
        schema.node("paragraph", null, [schema.text("hello")]),
      ]),
    });
    const view = {
      get state() {
        return state;
      },
      updateState(nextState: EditorState) {
        state = nextState;
      },
    } as Pick<EditorView, "state" | "updateState"> as EditorView;
    const onDocChanged = vi.fn();

    dispatchEditorTransaction({
      view,
      transaction: state.tr.setMeta("source", "test") as Transaction,
      onDocChanged,
    });

    expect(state.doc.textContent).toBe("hello");
    expect(onDocChanged).not.toHaveBeenCalled();
  });
});
