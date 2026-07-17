import { EditorState, type Transaction } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { describe, expect, it, vi } from "vitest";

import { schema } from "./schema";
import { handleTrailingEmptyLineMouseDown } from "./trailing-empty-line-click";

describe("handleTrailingEmptyLineMouseDown", () => {
  it("adds an empty paragraph when clicking below the last filled line", () => {
    const { view, getState } = createView([
      schema.node("paragraph", null, [schema.text("Follow up")]),
    ]);
    const event = createMouseEvent({ target: view.dom, clientY: 40 });

    const handled = handleTrailingEmptyLineMouseDown(view, event);

    expect(handled).toBe(true);
    expect(event.defaultPrevented).toBe(true);
    expect(getState().doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "Follow up" }],
        },
        { type: "paragraph" },
      ],
    });
    expect(getState().selection.from).toBe(getState().doc.content.size - 1);
    expect(view.focus).toHaveBeenCalled();
  });

  it("adds a body paragraph when clicking below a title-only document", () => {
    const { view, getState } = createView([
      schema.node("heading", { level: 1 }, [schema.text("Weekly sync")]),
    ]);
    const event = createMouseEvent({ target: view.dom, clientY: 40 });

    const handled = handleTrailingEmptyLineMouseDown(view, event);

    expect(handled).toBe(true);
    expect(getState().doc.toJSON()).toEqual({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Weekly sync" }],
        },
        { type: "paragraph" },
      ],
    });
    expect(getState().selection.from).toBe(getState().doc.content.size - 1);
  });

  it("uses an existing trailing empty paragraph instead of adding another", () => {
    const { view, getState } = createView([
      schema.node("paragraph", null, [schema.text("Follow up")]),
      schema.node("paragraph"),
    ]);
    const event = createMouseEvent({ target: view.dom, clientY: 40 });

    const handled = handleTrailingEmptyLineMouseDown(view, event);

    expect(handled).toBe(true);
    expect(getState().doc.childCount).toBe(2);
    expect(getState().selection.from).toBe(getState().doc.content.size - 1);
  });

  it("ignores clicks on document blocks", () => {
    const { view, block, getState } = createView([
      schema.node("paragraph", null, [schema.text("Follow up")]),
    ]);
    const event = createMouseEvent({ target: block, clientY: 10 });

    const handled = handleTrailingEmptyLineMouseDown(view, event);

    expect(handled).toBe(false);
    expect(getState().doc.childCount).toBe(1);
  });
});

function createView(children: Parameters<typeof schema.node>[2]) {
  const block = document.createElement("p");
  block.getBoundingClientRect = () =>
    ({
      bottom: 20,
      height: 20,
      left: 0,
      right: 100,
      top: 0,
      width: 100,
      x: 0,
      y: 0,
      toJSON: () => {},
    }) as DOMRect;

  const dom = document.createElement("div");
  dom.append(block);

  let state = EditorState.create({
    schema,
    doc: schema.node("doc", null, children),
  });

  const view = {
    dom,
    get state() {
      return state;
    },
    dispatch(transaction: Transaction) {
      state = state.apply(transaction);
    },
    focus: vi.fn(),
  } as Pick<EditorView, "dispatch" | "dom" | "focus" | "state"> as EditorView;

  return { view, block, getState: () => state };
}

function createMouseEvent({
  target,
  clientY,
}: {
  target: EventTarget;
  clientY: number;
}) {
  const event = new MouseEvent("mousedown", {
    button: 0,
    cancelable: true,
    clientY,
  });
  Object.defineProperty(event, "target", { value: target });
  return event;
}
