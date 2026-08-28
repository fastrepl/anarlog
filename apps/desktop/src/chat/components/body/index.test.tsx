import { cleanup, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { shellState } = vi.hoisted(() => ({
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
    isAtBottom: true,
    scrollRef: { current: null },
    scrollToBottom: vi.fn(),
    showGoToRecent: false,
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

describe("ChatBody", () => {
  beforeEach(() => {
    cleanup();
    shellState.mode = "FloatingOpen";
  });

  it("renders the floating body layout", () => {
    render(<ChatBody messages={[]} status="ready" />);

    const content = screen.getByTestId("chat-body-empty").parentElement;
    const scrollArea = content?.parentElement;
    const root = scrollArea?.parentElement as HTMLElement | null;

    expect(content?.hasAttribute("data-chat-body-content")).toBe(true);
    expect(scrollArea?.hasAttribute("data-chat-scroll-area")).toBe(true);
    expect(root?.dataset.chatBodyLayout).toBe("floating");
  });

  it("renders the right-panel body layout", () => {
    shellState.mode = "RightPanelOpen";

    render(<ChatBody messages={[]} status="ready" />);

    const content = screen.getByTestId("chat-body-empty").parentElement;
    const scrollArea = content?.parentElement;
    const root = scrollArea?.parentElement as HTMLElement | null;

    expect(content?.hasAttribute("data-chat-body-content")).toBe(true);
    expect(scrollArea?.hasAttribute("data-chat-scroll-area")).toBe(true);
    expect(root?.dataset.chatBodyLayout).toBe("right-panel");
  });
});
