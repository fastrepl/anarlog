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
    onRemoveContextEntity: vi.fn(),
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
    layout,
    onDraftContentChange,
    onOpenRightPanel,
    sessionProps,
  }: {
    layout?: "floating" | "right-panel";
    onDraftContentChange?: (hasDraftContent: boolean) => void;
    onOpenRightPanel?: () => void;
    sessionProps: unknown;
  }) => (
    <>
      <button
        data-has-session={String(sessionProps === mocks.sessionProps)}
        data-layout={layout}
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

  it("anchors the floating panel to the bottom center of the note surface", async () => {
    render(<TestHost />);

    await screen.findByTestId("chat-view");

    expect(screen.getByTestId("open-right-panel").dataset.hasSession).toBe(
      "true",
    );
    const resizeFrame = document.querySelector("[data-chat-resize-frame]");
    const panel = document.querySelector<HTMLElement>("[data-chat-panel]");

    await waitFor(() => {
      expect(resizeFrame?.className).toContain("items-end");
      expect(resizeFrame?.className).toContain("justify-center");
      expect(resizeFrame?.className).toContain("px-3");
      expect(resizeFrame?.className).toContain("pb-2");
      expect(resizeFrame?.className).not.toContain("pb-3");
      expect(panel?.style.width).toBe("calc(100% - 1.5rem)");
      expect(panel?.style.minWidth).toBe("min(368px, calc(100% - 1.5rem))");
      expect(panel?.style.maxWidth).toBe("648px");
      expect(panel?.style.height).toBe("");
      expect(panel?.style.transformOrigin).toBe("bottom center");
      expect(panel?.className).toContain("rounded-[20px]");
      expect(panel?.dataset.chatPanelReveal).toBe("bottom-up");
    });
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

    const resizeFrame = document.querySelector<HTMLElement>(
      "[data-chat-resize-frame]",
    );

    fireEvent.click(resizeFrame!);

    expect(mocks.sendEvent).toHaveBeenCalledWith({ type: "CLOSE" });
  });

  it("keeps the expanded composer open on backdrop click when draft has content", async () => {
    render(<TestHost />);

    await screen.findByTestId("chat-view");

    fireEvent.click(screen.getByTestId("mark-draft-content"));
    mocks.sendEvent.mockClear();

    const resizeFrame = document.querySelector<HTMLElement>(
      "[data-chat-resize-frame]",
    );

    fireEvent.click(resizeFrame!);

    expect(mocks.sendEvent).not.toHaveBeenCalledWith({ type: "CLOSE" });
  });

  it("resizes the bottom handle by the pointer movement", async () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
      function (this: HTMLElement) {
        if (this.matches("[data-chat-panel]")) {
          return {
            bottom: 600,
            height: 360,
            left: 240,
            right: 660,
            top: 240,
            width: 420,
            x: 240,
            y: 240,
            toJSON: () => ({}),
          };
        }

        return {
          bottom: 720,
          height: 720,
          left: 0,
          right: 900,
          top: 0,
          width: 900,
          x: 0,
          y: 0,
          toJSON: () => ({}),
        };
      },
    );

    render(<TestHost />);

    await screen.findByTestId("chat-view");

    const resizeFrame = document.querySelector<HTMLElement>(
      "[data-chat-resize-frame]",
    );
    const panel = document.querySelector<HTMLElement>("[data-chat-panel]");
    const bottomHandle = document.querySelector<HTMLElement>(
      '[data-chat-resize-handle="bottom"]',
    );

    expect(resizeFrame).toBeTruthy();
    expect(panel).toBeTruthy();
    expect(bottomHandle).toBeTruthy();

    fireEvent.pointerDown(bottomHandle!, {
      clientX: 450,
      clientY: 600,
      pointerId: 1,
    });
    fireEvent.pointerMove(bottomHandle!, {
      clientX: 450,
      clientY: 640,
      pointerId: 1,
    });

    expect(panel?.style.height).toBe("400px");
  });

  it("resizes from side, top, and top corner handles", async () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
      function (this: HTMLElement) {
        if (this.matches("[data-chat-panel]")) {
          return {
            bottom: 600,
            height: 360,
            left: 240,
            right: 660,
            top: 240,
            width: 420,
            x: 240,
            y: 240,
            toJSON: () => ({}),
          };
        }

        return {
          bottom: 720,
          height: 720,
          left: 0,
          right: 900,
          top: 0,
          width: 900,
          x: 0,
          y: 0,
          toJSON: () => ({}),
        };
      },
    );

    render(<TestHost />);

    await screen.findByTestId("chat-view");

    const panel = document.querySelector<HTMLElement>("[data-chat-panel]");
    expect(panel).toBeTruthy();

    const dragHandle = (
      handle: string,
      start: { x: number; y: number },
      end: { x: number; y: number },
    ) => {
      const resizeHandle = document.querySelector<HTMLElement>(
        `[data-chat-resize-handle="${handle}"]`,
      );

      expect(resizeHandle).toBeTruthy();

      fireEvent.pointerDown(resizeHandle!, {
        clientX: start.x,
        clientY: start.y,
        pointerId: 1,
      });
      fireEvent.pointerMove(resizeHandle!, {
        clientX: end.x,
        clientY: end.y,
        pointerId: 1,
      });
      fireEvent.pointerUp(resizeHandle!, {
        clientX: end.x,
        clientY: end.y,
        pointerId: 1,
      });
    };

    dragHandle("right", { x: 660, y: 420 }, { x: 700, y: 420 });
    expect(panel?.style.width).toBe("460px");
    expect(panel?.style.height).toBe("360px");

    dragHandle("left", { x: 240, y: 420 }, { x: 200, y: 420 });
    expect(panel?.style.width).toBe("460px");
    expect(panel?.style.height).toBe("360px");

    dragHandle("top", { x: 450, y: 240 }, { x: 450, y: 200 });
    expect(panel?.style.width).toBe("420px");
    expect(panel?.style.height).toBe("400px");

    dragHandle("top-left", { x: 240, y: 240 }, { x: 200, y: 200 });
    expect(panel?.style.width).toBe("460px");
    expect(panel?.style.height).toBe("400px");

    dragHandle("top-right", { x: 660, y: 240 }, { x: 700, y: 200 });
    expect(panel?.style.width).toBe("460px");
    expect(panel?.style.height).toBe("400px");
  });

  it("does not render bottom corner handles or visible corner indicators", async () => {
    render(<TestHost />);

    await screen.findByTestId("chat-view");

    expect(
      document.querySelector('[data-chat-resize-handle="bottom-left"]'),
    ).toBeNull();
    expect(
      document.querySelector('[data-chat-resize-handle="bottom-right"]'),
    ).toBeNull();
    expect(document.querySelector("[data-chat-resize-handle] span")).toBeNull();
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
