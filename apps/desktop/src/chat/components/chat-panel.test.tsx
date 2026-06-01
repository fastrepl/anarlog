import { cleanup, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  chat: {
    groupId: undefined as string | undefined,
    selectChat: vi.fn(),
    sessionId: "chat-session-id",
    setGroupId: vi.fn(),
    startNewChat: vi.fn(),
  },
  toolbarControls: vi.fn(),
}));

vi.mock("./toolbar-controls", () => ({
  ChatToolbarControls: (props: {
    layout?: "floating" | "right-panel";
    surface?: "light" | "dark";
  }) => {
    mocks.toolbarControls(props);
    return <div data-surface={props.surface} data-testid="chat-toolbar" />;
  },
}));

vi.mock("./use-session-tab", () => ({
  useSessionTab: () => ({ currentSessionId: "current-session-id" }),
}));

vi.mock("~/ai/hooks", () => ({
  useLanguageModel: () => undefined,
}));

vi.mock("~/chat/store/use-chat-actions", () => ({
  useChatActions: () => ({ handleSendMessage: vi.fn() }),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({ chat: mocks.chat }),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useValues: () => ({}),
  },
}));

import { ChatView } from "./chat-panel";

describe("ChatView", () => {
  beforeEach(() => {
    cleanup();
    mocks.toolbarControls.mockClear();
  });

  it("uses the modal dark surface for the right panel layout", () => {
    const { container } = render(<ChatView layout="right-panel" />);
    const root = container.firstElementChild;

    expect(root?.className).toContain("bg-stone-800");
    expect(root?.className).toContain("text-white");
    expect(root?.firstElementChild?.className).toContain("h-12");
    expect(screen.getByTestId("chat-toolbar").dataset.surface).toBe("dark");
    expect(mocks.toolbarControls).toHaveBeenCalledWith(
      expect.objectContaining({
        layout: "right-panel",
        surface: "dark",
      }),
    );
  });
});
