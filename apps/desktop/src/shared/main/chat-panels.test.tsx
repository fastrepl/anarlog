import { cleanup, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  chatMode: "FloatingClosed" as
    | "FloatingClosed"
    | "FloatingOpen"
    | "RightPanelOpen",
  persistentChatPanel: vi.fn(),
  sendEvent: vi.fn(),
}));

vi.mock("@hypr/ui/components/ui/resizable", () => ({
  ResizablePanelGroup: ({
    children,
    direction,
  }: {
    children: React.ReactNode;
    direction: string;
  }) => (
    <div data-direction={direction} data-testid="panel-group">
      {children}
    </div>
  ),
  ResizablePanel: ({
    children,
    className,
    defaultSize,
    maxSize,
    minSize,
    style,
  }: {
    children: React.ReactNode;
    className?: string;
    defaultSize?: number;
    maxSize?: number;
    minSize?: number;
    style?: React.CSSProperties;
  }) => (
    <div
      data-class-name={className}
      data-default-size={defaultSize}
      data-max-size={maxSize}
      data-min-size={minSize}
      data-min-width={style?.minWidth}
      data-testid="panel"
    >
      {children}
    </div>
  ),
  ResizableHandle: ({ className }: { className?: string }) => (
    <div data-class-name={className} data-testid="resize-handle" />
  ),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: mocks.chatMode,
      sendEvent: mocks.sendEvent,
    },
  }),
}));

vi.mock("~/chat/components/chat-panel", () => ({
  ChatView: ({
    layout,
    onOpenFloating,
  }: {
    layout?: "floating" | "right-panel";
    onOpenFloating?: () => void;
  }) => (
    <button
      data-layout={layout}
      data-testid="chat-view"
      type="button"
      onClick={onOpenFloating}
    >
      Chat
    </button>
  ),
}));

vi.mock("~/chat/components/persistent-chat", () => ({
  PersistentChatPanel: ({
    floatingContainerRef,
  }: {
    floatingContainerRef: { current: HTMLDivElement | null };
  }) => {
    mocks.persistentChatPanel(floatingContainerRef);
    return <div data-testid="persistent-chat-panel" />;
  },
}));

import { MainChatPanels } from "./chat-panels";

describe("MainChatPanels", () => {
  beforeEach(() => {
    cleanup();
    mocks.chatMode = "FloatingClosed";
    mocks.persistentChatPanel.mockClear();
    mocks.sendEvent.mockClear();
  });

  it("renders the main content and persistent floating chat host", () => {
    render(
      <MainChatPanels>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    expect(screen.getByTestId("main-content")).toBeTruthy();
    expect(screen.getByTestId("persistent-chat-panel")).toBeTruthy();
    expect(mocks.persistentChatPanel).toHaveBeenCalledTimes(1);
    expect(mocks.persistentChatPanel.mock.calls[0]?.[0].current).toBeInstanceOf(
      HTMLDivElement,
    );
    expect(screen.getByTestId("panel-group").dataset.direction).toBe(
      "horizontal",
    );
    expect(screen.queryByTestId("resize-handle")).toBeNull();
    expect(screen.getAllByTestId("panel")).toHaveLength(1);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("renders the right chat panel when chat is docked", () => {
    mocks.chatMode = "RightPanelOpen";

    render(
      <MainChatPanels>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    expect(screen.getAllByTestId("panel")).toHaveLength(2);
    expect(screen.getByTestId("resize-handle")).toBeTruthy();
    expect(screen.getByTestId("chat-view").dataset.layout).toBe("right-panel");
    const rightPanel = document.querySelector("[data-chat-right-panel]");

    expect(rightPanel).toBeInstanceOf(HTMLDivElement);
    expect(rightPanel?.className).toContain("bg-card");
    expect(rightPanel?.className).toContain("border-x");
    expect(rightPanel?.className).toContain("border-border");
    expect(rightPanel?.className).not.toContain("border-b-0");
    expect(rightPanel?.className).toContain("rounded-tr-xl");
    expect(rightPanel?.className).not.toContain("rounded-t-xl");
    expect(rightPanel?.className).not.toContain("ml-2");
    expect(rightPanel?.className).not.toContain("mr-1");
  });
});
