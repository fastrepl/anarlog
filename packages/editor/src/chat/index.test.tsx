// @vitest-environment jsdom

import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { createElement, createRef } from "react";
import {
  afterAll,
  afterEach,
  beforeAll,
  describe,
  expect,
  it,
  vi,
} from "vitest";

import { ChatEditor, type ChatEditorHandle } from "./index";

class NoopResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}

beforeAll(() => {
  Object.defineProperty(Range.prototype, "getClientRects", {
    configurable: true,
    value: () => [],
  });
  Object.defineProperty(Range.prototype, "getBoundingClientRect", {
    configurable: true,
    value: () => new DOMRect(),
  });
  vi.stubGlobal("ResizeObserver", NoopResizeObserver);
});

afterAll(() => {
  Reflect.deleteProperty(Range.prototype, "getClientRects");
  Reflect.deleteProperty(Range.prototype, "getBoundingClientRect");
  vi.unstubAllGlobals();
});

afterEach(cleanup);

describe("ChatEditor slash menu", () => {
  it("inserts the chosen prompt as a chip on Enter without sending, while Enter still sends when nothing matches", async () => {
    const ref = createRef<ChatEditorHandle>();
    const onSubmit = vi.fn();
    const rendered = render(
      createElement(ChatEditor, {
        ref,
        submitShortcut: "enter",
        onSubmit,
        slashConfig: {
          trigger: "/",
          handleSearch: async (query: string) =>
            "what should i say?".startsWith(query.toLowerCase())
              ? [
                  {
                    id: "prompt-1",
                    type: "chat_prompt",
                    label: "What should I say?",
                  },
                ]
              : [],
        },
      }),
    );
    await waitFor(() => expect(ref.current?.focus()).toBe(true));
    const surface = rendered.getByRole("textbox");

    act(() => ref.current?.insertText("/"));
    await screen.findByText("What should I say?");
    fireEvent.keyDown(surface, { key: "Enter" });

    expect(onSubmit).not.toHaveBeenCalled();
    expect(ref.current?.getJSON()?.content?.[0]?.content?.[0]).toMatchObject({
      type: "mention-@",
      attrs: {
        id: "prompt-1",
        type: "chat_prompt",
        label: "What should I say?",
      },
    });

    act(() => ref.current?.insertText("/etc"));
    fireEvent.keyDown(surface, { key: "Enter" });

    expect(onSubmit).toHaveBeenCalledTimes(1);
  });
});
