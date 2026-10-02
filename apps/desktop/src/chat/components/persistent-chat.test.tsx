import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { useRef } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  chatMode: {
    current: "FloatingOpen" as
      | "FloatingClosed"
      | "FloatingOpen"
      | "RightPanelOpen",
  },
  sendEvent: vi.fn(),
  sessionProps: {
    contextEntities: [],
    isSystemPromptReady: true,
    messages: [],
    onAddContextEntity: vi.fn(),
    onDraftContextRefsChange: vi.fn(),
    pendingRefs: [],
    regenerate: vi.fn(),
    sendMessage: vi.fn(),
    sessionId: "chat-session-1",
    setMessages: vi.fn(),
    status: "ready" as const,
    stop: vi.fn(),
  },
}));

vi.mock("react-hotkeys-hook", () => ({
  useHotkeys: vi.fn(),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: mocks.chatMode.current,
      sendEvent: mocks.sendEvent,
    },
  }),
}));

vi.mock("./chat-panel", () => ({
  ChatPanelFrame: ({
    onDraftContentChange,
    onOpenRightPanel,
  }: {
    onDraftContentChange?: (hasDraftContent: boolean) => void;
    onOpenRightPanel?: () => void;
  }) => (
    <>
      <button
        data-testid="open-right-panel"
        type="button"
        onClick={onOpenRightPanel}
      >
        Open right panel
      </button>
      <button
        data-testid="mark-draft-content"
        type="button"
        onClick={() => onDraftContentChange?.(true)}
      >
        Mark draft content
      </button>
      <div data-testid="chat-view" />
    </>
  ),
}));

import { PersistentChatPanel } from "./persistent-chat";

function TestHost() {
  const containerRef = useRef<HTMLDivElement>(null);

  return (
    <div ref={containerRef} data-testid="full-panel-container">
      <div data-chat-floating-anchor />
      <PersistentChatPanel
        floatingContainerRef={containerRef}
        sessionProps={mocks.sessionProps}
      />
    </div>
  );
}

describe("PersistentChatPanel", () => {
  beforeEach(() => {
    cleanup();
    mocks.chatMode.current = "FloatingOpen";
    mocks.sendEvent.mockClear();
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as typeof ResizeObserver;
  });

  it("opens the docked right panel from the toolbar action", async () => {
    render(<TestHost />);

    await screen.findByTestId("chat-view");

    fireEvent.click(screen.getByTestId("open-right-panel"));

    expect(mocks.sendEvent).toHaveBeenCalledWith({
      type: "OPEN_RIGHT_PANEL",
    });
  });

  it("closes on backdrop click while the draft is empty", async () => {
    render(<TestHost />);

    await screen.findByTestId("chat-view");

    const floatingFrame = document.querySelector<HTMLElement>(
      "[data-chat-floating-frame]",
    );

    fireEvent.click(floatingFrame!);

    expect(mocks.sendEvent).toHaveBeenCalledWith({ type: "CLOSE" });
  });

  it("keeps the expanded composer open on backdrop click when draft has content", async () => {
    render(<TestHost />);

    await screen.findByTestId("chat-view");

    fireEvent.click(screen.getByTestId("mark-draft-content"));
    mocks.sendEvent.mockClear();

    const floatingFrame = document.querySelector<HTMLElement>(
      "[data-chat-floating-frame]",
    );

    fireEvent.click(floatingFrame!);

    expect(mocks.sendEvent).not.toHaveBeenCalledWith({ type: "CLOSE" });
  });

  it("hides the floating panel when the chat moves to the right panel", async () => {
    const { rerender } = render(<TestHost />);

    await screen.findByTestId("chat-view");

    mocks.chatMode.current = "RightPanelOpen";
    rerender(<TestHost />);

    await waitFor(() => {
      expect(
        document.querySelector<HTMLElement>("[data-chat-panel]"),
      ).toBeNull();
    });
  });
});
