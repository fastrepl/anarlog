import { EditorState } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { afterEach, describe, expect, it, vi } from "vitest";

import { schema } from "../note/schema";
import { getOpenableHref, linkOpenPlugin } from "./link-open";

const views: EditorView[] = [];

afterEach(() => {
  for (const view of views) {
    view.destroy();
  }
  views.length = 0;
  document.body.innerHTML = "";
});

function createLinkedEditor(href: string, onOpen = vi.fn()) {
  const mark = schema.marks.link.create({ href, target: null });
  const doc = schema.node("doc", null, [
    schema.node("paragraph", null, [schema.text("Open link", [mark])]),
  ]);
  const state = EditorState.create({
    doc,
    plugins: [linkOpenPlugin(onOpen)],
  });
  const host = document.createElement("div");
  document.body.append(host);
  const view = new EditorView(host, { state });
  views.push(view);

  const anchor = view.dom.querySelector("a[href]");
  if (!anchor) {
    throw new Error("Expected editor to render a link");
  }

  return { anchor, onOpen };
}

describe("linkOpenPlugin", () => {
  it("opens clicked http links", () => {
    const { anchor, onOpen } = createLinkedEditor(
      "https://linear.app/example/path",
    );
    const event = new MouseEvent("click", {
      bubbles: true,
      cancelable: true,
    });

    const defaultAllowed = anchor.dispatchEvent(event);

    expect(defaultAllowed).toBe(false);
    expect(onOpen).toHaveBeenCalledWith("https://linear.app/example/path");
  });

  it("blocks unsafe link schemes", () => {
    const { anchor, onOpen } = createLinkedEditor("javascript:alert(1)");
    const event = new MouseEvent("click", {
      bubbles: true,
      cancelable: true,
    });

    const defaultAllowed = anchor.dispatchEvent(event);

    expect(defaultAllowed).toBe(false);
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("allows mention anchors to handle their own clicks", () => {
    const { onOpen } = createLinkedEditor("https://linear.app/example/path");
    const mention = document.createElement("a");
    const bubbleHandler = vi.fn();
    mention.href = "javascript:void(0)";
    mention.dataset.mention = "true";
    mention.textContent = "Alex";
    views[0]!.dom.append(mention);
    document.body.addEventListener("click", bubbleHandler);

    const defaultAllowed = mention.dispatchEvent(
      new MouseEvent("click", {
        bubbles: true,
        cancelable: true,
      }),
    );

    expect(defaultAllowed).toBe(true);
    expect(bubbleHandler).toHaveBeenCalledOnce();
    expect(onOpen).not.toHaveBeenCalled();
    document.body.removeEventListener("click", bubbleHandler);
  });
});

describe("getOpenableHref", () => {
  it("allows http and https URLs", () => {
    expect(getOpenableHref("http://localhost:3000")).toBe(
      "http://localhost:3000/",
    );
    expect(getOpenableHref("https://example.com/docs")).toBe(
      "https://example.com/docs",
    );
  });

  it("rejects non-web URLs", () => {
    expect(getOpenableHref("mailto:hello@example.com")).toBeNull();
    expect(getOpenableHref("file:///tmp/note.txt")).toBeNull();
    expect(getOpenableHref("/relative/path")).toBeNull();
  });
});
