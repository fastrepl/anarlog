import { cleanup, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  toolbarControls: vi.fn((_props: Record<string, unknown>) => (
    <div data-testid="chat-toolbar" />
  )),
  chat: {
    groupId: "group-1",
    sessionId: "session-1",
    startNewChat: vi.fn(),
    selectChat: vi.fn(),
  },
}));

vi.mock("./toolbar-controls", () => ({
  ChatToolbarControls: (props: Record<string, unknown>) => {
    mocks.toolbarControls(props);
    return (
      <div data-surface={props.surface as string} data-testid="chat-toolbar" />
    );
  },
}));

vi.mock("./body", () => ({
  ChatBody: () => <div data-testid="chat-body" />,
}));

vi.mock("./content", () => ({
  ChatContent: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));

vi.mock("./session-provider", () => ({
  ChatSession: ({
    children,
  }: {
    children: (props: object) => React.ReactNode;
  }) =>
    children({
      messages: [],
      status: "ready",
      error: undefined,
      regenerate: vi.fn(),
      contextEntities: [],
      sendMessage: vi.fn(),
      pendingRefs: [],
    }),
}));

vi.mock("~/ai/hooks", () => ({
  useLanguageModel: () => ({ id: "model-1" }),
}));

vi.mock("~/chat/store/use-chat-actions", () => ({
  useChatActions: () => ({
    handleSendMessage: vi.fn(),
  }),
}));

vi.mock("./use-session-tab", () => ({
  useSessionTab: () => ({ currentSessionId: "session-1" }),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({ chat: mocks.chat }),
}));

import { ChatView } from "./chat-panel";

describe("ChatView", () => {
  beforeEach(() => {
    cleanup();
    mocks.toolbarControls.mockClear();
  });

  it("uses the sidebar card shell in the right panel layout", () => {
    const { container } = render(<ChatView layout="right-panel" />);
    const root = container.firstElementChild;

    expect(root?.className).toContain("bg-card");
    expect(root?.className).toContain("text-card-foreground");
    expect(root?.className).toContain("h-full");
    expect(root?.className).not.toContain("bg-primary");
    expect(root?.firstElementChild?.className).toContain("h-12");
    expect(root?.firstElementChild?.className).not.toContain("border-b");
    expect(screen.getByTestId("chat-toolbar").dataset.surface).toBe("light");
    expect(mocks.toolbarControls).toHaveBeenCalledWith(
      expect.objectContaining({
        layout: "right-panel",
        onClose: expect.any(Function),
        surface: "light",
      }),
    );
  });

  it("uses the neutral shell in the floating layout", () => {
    const { container } = render(<ChatView layout="floating" />);
    const root = container.firstElementChild;

    expect(root?.className).toContain("bg-[#f4f4f5]");
    expect(root?.className).toContain("text-card-foreground");
    expect(root?.className).toContain("max-h-full");
    expect(root?.className).not.toContain("bg-card");
    expect(root?.className.split(" ")).not.toContain("h-full");
    expect(root?.firstElementChild?.className).toContain("h-11");
    expect(root?.firstElementChild?.className).not.toContain("border-b");
    expect(screen.getByTestId("chat-toolbar").dataset.surface).toBe("light");
  });
});
