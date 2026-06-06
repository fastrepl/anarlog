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

const appearanceState = vi.hoisted(() => ({
  isDarkAppearance: true,
}));

vi.mock("~/chat/hooks/use-chat-appearance", () => ({
  useChatAppearance: () => ({
    isDarkAppearance: appearanceState.isDarkAppearance,
    toolbarSurface: appearanceState.isDarkAppearance ? "dark" : "light",
    panelClassName: appearanceState.isDarkAppearance
      ? "bg-primary text-primary-foreground"
      : "bg-card text-foreground",
    panelBorderClassName: appearanceState.isDarkAppearance
      ? "border-primary/80"
      : "border-border",
    elevatedSurfaceClassName: appearanceState.isDarkAppearance
      ? "bg-primary-foreground/95 text-primary"
      : "bg-muted text-foreground",
    inputEditorClassName: appearanceState.isDarkAppearance
      ? "text-primary"
      : "text-foreground",
  }),
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
    appearanceState.isDarkAppearance = true;
  });

  it("uses the dark stone surface when the app is in dark appearance", () => {
    const { container } = render(<ChatView layout="right-panel" />);
    const root = container.firstElementChild;

    expect(root?.className).toContain("bg-primary");
    expect(root?.className).toContain("text-primary-foreground");
    expect(root?.firstElementChild?.className).toContain("h-12");
    expect(screen.getByTestId("chat-toolbar").dataset.surface).toBe("dark");
    expect(mocks.toolbarControls).toHaveBeenCalledWith(
      expect.objectContaining({
        layout: "right-panel",
        surface: "dark",
      }),
    );
  });

  it("uses the light card surface when the app is in light appearance", () => {
    appearanceState.isDarkAppearance = false;

    const { container } = render(<ChatView layout="right-panel" />);
    const root = container.firstElementChild;

    expect(root?.className).toContain("bg-card");
    expect(root?.className).toContain("text-foreground");
    expect(screen.getByTestId("chat-toolbar").dataset.surface).toBe("light");
  });
});
