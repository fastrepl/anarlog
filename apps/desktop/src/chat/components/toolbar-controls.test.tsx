import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  useRecentChatGroups: vi.fn(() => []),
}));

vi.mock("@anlg/ui/components/ui/button", () => ({
  Button: ({
    children,
    size: _size,
    sx,
    variant: _variant,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement> & {
    size?: string;
    sx?: unknown;
    variant?: string;
  }) => (
    <button data-stylex={String(Boolean(sx))} type="button" {...props}>
      {children}
    </button>
  ),
}));

vi.mock("@anlg/ui/components/ui/dropdown-menu", () => ({
  AppFloatingPanel: ({
    children,
    sx,
  }: {
    children: ReactNode;
    sx?: unknown;
  }) => (
    <div data-stylex={String(Boolean(sx))} data-testid="chat-history-panel">
      {children}
    </div>
  ),
  DropdownMenu: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuContent: ({
    avoidCollisions,
    children,
    collisionPadding,
    side,
    sideOffset,
    sx,
  }: {
    avoidCollisions?: boolean;
    children: ReactNode;
    collisionPadding?: number;
    side?: string;
    sideOffset?: number;
    sx?: unknown;
  }) => (
    <div
      data-avoid-collisions={String(avoidCollisions)}
      data-collision-padding={collisionPadding}
      data-side={side}
      data-side-offset={sideOffset}
      data-stylex={String(Boolean(sx))}
      data-testid="chat-history-menu"
    >
      {children}
    </div>
  ),
  DropdownMenuTrigger: ({ children }: { children: ReactNode }) => (
    <>{children}</>
  ),
}));

vi.mock("~/chat/store/queries", () => ({
  useRecentChatGroups: mocks.useRecentChatGroups,
}));

import { ChatToolbarControls } from "./toolbar-controls";

describe("ChatToolbarControls", () => {
  beforeEach(() => {
    cleanup();
    mocks.useRecentChatGroups.mockClear();
  });

  it("renders the dark chat history trigger through StyleX", () => {
    render(
      <ChatToolbarControls
        chatScope="general"
        currentChatGroupId={undefined}
        onNewChat={vi.fn()}
        onOpenRightPanel={vi.fn()}
        onSelectChat={vi.fn()}
        surface="dark"
      />,
    );

    const historyButton = screen.getByRole("button", { name: "Chat history" });
    expect(historyButton.dataset.stylex).toBe("true");
    expect(screen.queryByText("Ask Anarlog AI anything")).toBeNull();
  });

  it("renders the light chat history trigger without title text", () => {
    const { container } = render(
      <ChatToolbarControls
        chatScope="general"
        currentChatGroupId={undefined}
        onNewChat={vi.fn()}
        onOpenRightPanel={vi.fn()}
        onSelectChat={vi.fn()}
        surface="light"
      />,
    );

    const historyButton = screen.getByRole("button", { name: "Chat history" });
    expect(
      (container.firstElementChild as HTMLElement | null)?.dataset
        .chatToolbarLayout,
    ).toBe("floating");
    expect(historyButton.dataset.stylex).toBe("true");
    expect(historyButton.textContent).toBe("");
    expect(screen.queryByText("Ask Anarlog AI anything")).toBeNull();
  });

  it("opens floating chat history to the right and adapts to viewport collisions", () => {
    render(
      <ChatToolbarControls
        chatScope="general"
        currentChatGroupId={undefined}
        layout="floating"
        onNewChat={vi.fn()}
        onOpenRightPanel={vi.fn()}
        onSelectChat={vi.fn()}
        surface="light"
      />,
    );

    const menu = screen.getByTestId("chat-history-menu");
    const panel = screen.getByTestId("chat-history-panel");

    expect(menu.dataset.side).toBe("right");
    expect(menu.dataset.sideOffset).toBe("4");
    expect(menu.dataset.avoidCollisions).toBe("true");
    expect(menu.dataset.collisionPadding).toBe("8");
    expect(menu.dataset.stylex).toBe("true");
    expect(panel.dataset.stylex).toBe("true");
  });

  it("keeps right-panel chat history below the trigger", () => {
    render(
      <ChatToolbarControls
        chatScope="general"
        currentChatGroupId={undefined}
        layout="right-panel"
        onNewChat={vi.fn()}
        onOpenFloating={vi.fn()}
        onSelectChat={vi.fn()}
        surface="light"
      />,
    );

    expect(screen.getByTestId("chat-history-menu").dataset.side).toBe("bottom");
  });

  it("renders dark toolbar action buttons through StyleX without tooltips", () => {
    render(
      <ChatToolbarControls
        chatScope="general"
        currentChatGroupId={undefined}
        onClose={vi.fn()}
        onNewChat={vi.fn()}
        onOpenRightPanel={vi.fn()}
        onSelectChat={vi.fn()}
        surface="dark"
      />,
    );

    const newChatButton = screen.getByRole("button", { name: "New chat" });
    const rightPanelButton = screen.getByRole("button", {
      name: "Open in right panel",
    });

    expect(newChatButton.dataset.stylex).toBe("true");
    expect(newChatButton.getAttribute("title")).toBeNull();
    expect(rightPanelButton.dataset.stylex).toBe("true");
    expect(rightPanelButton.getAttribute("title")).toBeNull();
    expect(screen.queryByRole("button", { name: "Close chat" })).toBeNull();
  });

  it("renders floating toolbar actions without a close button", () => {
    const onClose = vi.fn();
    const onOpenRightPanel = vi.fn();

    const { container } = render(
      <ChatToolbarControls
        chatScope="general"
        currentChatGroupId={undefined}
        layout="floating"
        onClose={onClose}
        onNewChat={vi.fn()}
        onOpenRightPanel={onOpenRightPanel}
        onSelectChat={vi.fn()}
        surface="light"
      />,
    );

    const rightPanelButton = screen.getByRole("button", {
      name: "Open in right panel",
    });
    const actions = container.querySelector("[data-chat-toolbar-actions]");

    fireEvent.click(rightPanelButton);

    expect(actions).not.toBeNull();
    expect(rightPanelButton.dataset.stylex).toBe("true");
    expect(onOpenRightPanel).toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Close chat" })).toBeNull();
  });

  it("uses sidebar-matched toolbar metrics in the right panel", () => {
    const onClose = vi.fn();
    const onOpenFloating = vi.fn();
    const { container } = render(
      <ChatToolbarControls
        chatScope="general"
        currentChatGroupId={undefined}
        layout="right-panel"
        onClose={onClose}
        onNewChat={vi.fn()}
        onOpenFloating={onOpenFloating}
        onSelectChat={vi.fn()}
        surface="light"
      />,
    );

    const historyButton = screen.getByRole("button", { name: "Chat history" });
    const toolbar = container.firstElementChild as HTMLElement | null;

    expect(toolbar?.dataset.chatToolbarLayout).toBe("right-panel");
    expect(toolbar?.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(
      toolbar?.firstElementChild?.hasAttribute("data-tauri-drag-region"),
    ).toBe(true);
    const actions = container.querySelector("[data-chat-toolbar-actions]");
    expect(actions?.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(historyButton.dataset.stylex).toBe("true");
    expect(screen.queryByText("Ask Anarlog AI anything")).toBeNull();
    const floatButton = screen.getByRole("button", { name: "Float chat" });
    const closeButton = screen.getByRole("button", { name: "Close chat" });
    expect(historyButton.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(floatButton.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(closeButton.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(floatButton.dataset.stylex).toBe("true");
    expect(closeButton.dataset.stylex).toBe("true");
    expect(
      screen.queryByRole("button", { name: "Open in right panel" }),
    ).toBeNull();

    fireEvent.click(floatButton);
    fireEvent.click(closeButton);

    expect(onOpenFloating).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it("loads history for the active chat scope", () => {
    render(
      <ChatToolbarControls
        chatScope="automations"
        currentChatGroupId={undefined}
        onNewChat={vi.fn()}
        onOpenRightPanel={vi.fn()}
        onSelectChat={vi.fn()}
      />,
    );

    expect(mocks.useRecentChatGroups).toHaveBeenCalledWith("automations", 5);
  });
});
