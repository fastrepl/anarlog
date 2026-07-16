// @vitest-environment jsdom

import { act, cleanup, render, waitFor } from "@testing-library/react";
import { EditorState, TextSelection } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { createElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { JSONContent } from "./index";
import {
  createReadOnlyPlugin,
  getEditorCompositionWaitMs,
  NoteEditor,
  shouldReplaceEditorContent,
} from "./index";
import { schema } from "./schema";

const baseDoc: JSONContent = {
  type: "doc",
  content: [{ type: "paragraph", content: [{ type: "text", text: "old" }] }],
};

const nextDoc: JSONContent = {
  type: "doc",
  content: [{ type: "paragraph", content: [{ type: "text", text: "new" }] }],
};

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("shouldReplaceEditorContent", () => {
  it("does not replace content while IME composition is active", () => {
    expect(
      shouldReplaceEditorContent({
        currentContent: baseDoc,
        nextContent: nextDoc,
        hasFocus: true,
        isComposing: true,
        syncContentWhenFocused: true,
      }),
    ).toBe(false);
  });

  it("allows focused content sync after composition ends when enabled", () => {
    expect(
      shouldReplaceEditorContent({
        currentContent: baseDoc,
        nextContent: nextDoc,
        hasFocus: true,
        isComposing: false,
        syncContentWhenFocused: true,
      }),
    ).toBe(true);
  });
});

describe("getEditorCompositionWaitMs", () => {
  it("waits through active IME composition", () => {
    expect(
      getEditorCompositionWaitMs(
        { composing: false },
        { active: true, endedAt: 0 },
      ),
    ).toBe(500);
  });

  it("returns the remaining post-composition grace window", () => {
    vi.useFakeTimers();
    vi.setSystemTime(1000);

    expect(
      getEditorCompositionWaitMs(
        { composing: false },
        { active: false, endedAt: 600 },
      ),
    ).toBe(100);
  });

  it("returns zero after the post-composition grace window", () => {
    vi.useFakeTimers();
    vi.setSystemTime(1000);

    expect(
      getEditorCompositionWaitMs(
        { composing: false },
        { active: false, endedAt: 499 },
      ),
    ).toBe(0);
  });

  it("returns zero for a reset inactive composition state", () => {
    expect(
      getEditorCompositionWaitMs(
        { composing: false },
        { active: false, endedAt: 0 },
      ),
    ).toBe(0);
  });
});

describe("createReadOnlyPlugin", () => {
  it("rejects document changes while allowing selection changes", () => {
    const plugin = createReadOnlyPlugin();
    const state = EditorState.create({
      schema,
      doc: schema.node("doc", null, [
        schema.node("paragraph", null, [schema.text("shared")]),
      ]),
      plugins: [plugin],
    });

    expect(
      state.applyTransaction(state.tr.insertText("blocked")).transactions,
    ).toHaveLength(0);

    const selection = TextSelection.create(state.doc, 2);
    expect(
      state.applyTransaction(state.tr.setSelection(selection)).transactions,
    ).toHaveLength(1);
  });

  it("wires the editor surface to reject document changes", async () => {
    let view: EditorView | null = null;
    const handleChange = vi.fn();
    const rendered = render(
      createElement(NoteEditor, {
        initialContent: {
          type: "doc",
          content: [
            {
              type: "paragraph",
              content: [{ type: "text", text: "shared" }],
            },
          ],
        },
        handleChange,
        onViewReady: (nextView) => {
          view = nextView;
        },
        readOnly: true,
      }),
    );

    await waitFor(() => expect(view).not.toBeNull());
    const surface = rendered.getByRole("document");
    expect(surface.getAttribute("contenteditable")).toBe("false");
    expect(surface.getAttribute("aria-readonly")).toBe("true");

    act(() => {
      view?.dispatch(view.state.tr.insertText("blocked", 2));
    });

    expect(view?.state.doc.textContent).toBe("shared");
    expect(handleChange).not.toHaveBeenCalled();
  });
});
