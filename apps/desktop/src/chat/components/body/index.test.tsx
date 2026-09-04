import { cleanup, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { autoScrollState, shellState } = vi.hoisted(() => ({
  autoScrollState: {
    isAtBottom: true,
    showGoToRecent: false,
  },
  shellState: {
    mode: "FloatingOpen" as
      | "FloatingClosed"
      | "FloatingOpen"
      | "RightPanelOpen",
  },
}));

vi.mock("./empty", () => ({
  ChatBodyEmpty: () => <div data-testid="chat-body-empty" />,
}));

vi.mock("./non-empty", () => ({
  ChatBodyNonEmpty: () => <div data-testid="chat-body-non-empty" />,
}));

vi.mock("./use-chat-auto-scroll", () => ({
  useChatAutoScroll: () => ({
    contentRef: { current: null },
    handleKeyDown: vi.fn(),
    handlePointerDown: vi.fn(),
    handlePointerMove: vi.fn(),
    handleWheel: vi.fn(),
    isAtBottom: autoScrollState.isAtBottom,
    scrollRef: { current: null },
    scrollToBottom: vi.fn(),
    showGoToRecent: autoScrollState.showGoToRecent,
    updateAutoScrollState: vi.fn(),
  }),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: shellState.mode,
    },
  }),
}));

import { ChatBody } from "./index";

import type { AnlgUIMessage } from "~/chat/types";

describe("ChatBody", () => {
  beforeEach(() => {
    cleanup();
    autoScrollState.isAtBottom = true;
    autoScrollState.showGoToRecent = false;
    shellState.mode = "FloatingOpen";
  });

  it("keeps horizontal content padding", () => {
    render(<ChatBody messages={[]} status="ready" />);

    const content = screen.getByTestId("chat-body-empty").parentElement;
    const scrollArea = content?.parentElement;
    const root = scrollArea?.parentElement;

    expect(content?.className).toContain("px-5");
    expect(content?.className).not.toContain("min-h-full");
    expect(scrollArea?.className).toContain("max-h-[min(36rem,70vh)]");
    expect(scrollArea?.className).toContain("flex-auto");
    expect(root?.className).toContain("flex-auto");
    expect(root?.className).not.toContain("shrink-0");
    expect(content?.className).not.toContain("px-2");
    expect(content?.className).not.toContain("pr-0");
  });

  it("uses balanced content padding in the right panel", () => {
    shellState.mode = "RightPanelOpen";

    render(<ChatBody messages={[]} status="ready" />);

    const content = screen.getByTestId("chat-body-empty").parentElement;
    const scrollArea = content?.parentElement;
    const root = scrollArea?.parentElement;

    expect(content?.className).toContain("px-3");
    expect(content?.className).toContain("py-5");
    expect(content?.className).toContain("min-h-full");
    expect(scrollArea?.className).toContain("flex-1");
    expect(root?.className).toContain("flex-1");
    expect(content?.className).not.toContain("px-5");
    expect(content?.className).not.toContain("px-2");
  });

  it("positions the go-to-recent control without offsetting its visual effects", () => {
    autoScrollState.isAtBottom = false;
    autoScrollState.showGoToRecent = true;

    render(
      <ChatBody
        messages={[
          {
            id: "assistant-1",
            role: "assistant",
            parts: [{ type: "text", text: "Recent response" }],
          } as AnlgUIMessage,
        ]}
        status="ready"
      />,
    );

    const button = screen.getByRole("button", { name: "Go to recent" });
    const anchor = button.parentElement;

    expect(anchor?.hasAttribute("data-chat-go-to-recent-anchor")).toBe(true);
    expect(anchor?.className).toContain("-translate-x-1/2");
    expect(button.className).not.toContain("-translate-x-1/2");
    expect(button.className).not.toContain("transform");
  });
});
