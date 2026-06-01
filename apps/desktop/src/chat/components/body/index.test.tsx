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

  it("keeps horizontal content padding", () => {
    render(<ChatBody messages={[]} status="ready" />);

    const content = screen.getByTestId("chat-body-empty").parentElement;

    expect(content?.className).toContain("px-2");
    expect(content?.className).not.toContain("pr-0");
  });

  it("uses balanced content padding in the right panel", () => {
    shellState.mode = "RightPanelOpen";

    render(<ChatBody messages={[]} status="ready" />);

    const content = screen.getByTestId("chat-body-empty").parentElement;

    expect(content?.className).toContain("px-3");
    expect(content?.className).toContain("py-5");
    expect(content?.className).not.toContain("px-5");
    expect(content?.className).not.toContain("px-2");
  });
});
