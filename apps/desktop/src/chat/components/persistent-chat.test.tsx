import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { useRef, type ComponentProps, type ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  chatMode: "FloatingClosed" as "FloatingClosed" | "FloatingOpen",
  sendEvent: vi.fn(),
}));

vi.mock("motion/react", async () => {
  const React = await vi.importActual<typeof import("react")>("react");

  return {
    AnimatePresence: ({ children }: { children: ReactNode }) => <>{children}</>,
    motion: {
      div: React.forwardRef<
        HTMLDivElement,
        ComponentProps<"div"> & {
          animate?: unknown;
          exit?: unknown;
          initial?: unknown;
          transition?: unknown;
        }
      >(function MotionDiv(
        {
          animate: _animate,
          children,
          exit: _exit,
          initial: _initial,
          transition: _transition,
          ...props
        },
        ref,
      ) {
        return (
          <div ref={ref} {...props}>
            {children}
          </div>
        );
      }),
    },
  };
});

vi.mock("react-hotkeys-hook", () => ({
  useHotkeys: vi.fn(),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: mocks.chatMode,
      sendEvent: mocks.sendEvent,
    },
  }),
}));

vi.mock("./chat-panel", () => ({
  ChatView: () => <div data-testid="chat-modal-content" />,
}));

import { PersistentChatPanel } from "./persistent-chat";

describe("PersistentChatPanel", () => {
  beforeEach(() => {
    cleanup();
    mocks.chatMode = "FloatingClosed";
    mocks.sendEvent.mockClear();
    vi.restoreAllMocks();
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as typeof ResizeObserver;
  });

  it("stays hidden while the floating chat is closed", () => {
    render(<PersistentChatPanelHarness />);

    expect(screen.queryByTestId("chat-modal-content")).toBeNull();
  });

  it("renders the historical floating chat modal inside the main area", async () => {
    mocks.chatMode = "FloatingOpen";

    render(<PersistentChatPanelHarness />);

    await waitFor(() => {
      expect(screen.getByTestId("chat-modal-content")).toBeTruthy();
    });
  });

  it("closes from the floating backdrop", async () => {
    mocks.chatMode = "FloatingOpen";

    render(<PersistentChatPanelHarness />);

    const content = await screen.findByTestId("chat-modal-content");
    const backdrop = content.parentElement?.parentElement;
    expect(backdrop).toBeTruthy();

    fireEvent.click(backdrop!);

    expect(mocks.sendEvent).toHaveBeenCalledWith({ type: "CLOSE" });
  });

  it("resizes the floating chat modal from the bottom corner", async () => {
    mocks.chatMode = "FloatingOpen";
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
      function getBoundingClientRect(this: HTMLElement) {
        if (this.hasAttribute("data-chat-floating-anchor")) {
          return createRect({ width: 1000, height: 800 });
        }

        return createRect();
      },
    );

    render(<PersistentChatPanelHarness />);

    const content = await screen.findByTestId("chat-modal-content");
    const panel = content.closest("[data-chat-panel]") as HTMLDivElement;
    const frame = document.querySelector(
      "[data-chat-resize-frame]",
    ) as HTMLDivElement;
    const handle = panel.querySelector(
      '[data-chat-resize-handle="bottom-right"]',
    ) as HTMLDivElement;

    Object.defineProperty(panel, "getBoundingClientRect", {
      configurable: true,
      value: () => createRect({ left: 180, top: 120, width: 640, height: 560 }),
    });
    Object.defineProperty(frame, "getBoundingClientRect", {
      configurable: true,
      value: () => createRect({ width: 1000, height: 800 }),
    });

    fireEvent.pointerDown(handle, {
      pointerId: 1,
      buttons: 1,
      clientX: 0,
      clientY: 0,
    });
    fireEvent.pointerMove(handle, {
      pointerId: 1,
      buttons: 1,
      clientX: 100,
      clientY: 50,
    });

    await waitFor(() => {
      expect(panel.style.width).toBe("840px");
      expect(panel.style.height).toBe("660px");
      expect(frame.className).toContain("items-center");
    });
  });
});

function PersistentChatPanelHarness() {
  const ref = useRef<HTMLDivElement>(null);

  return (
    <div ref={ref}>
      <div data-chat-floating-anchor />
      <PersistentChatPanel floatingContainerRef={ref} />
    </div>
  );
}

function createRect({
  left = 0,
  top = 0,
  width = 0,
  height = 0,
}: Partial<DOMRect> = {}): DOMRect {
  return {
    x: left,
    y: top,
    left,
    top,
    width,
    height,
    right: left + width,
    bottom: top + height,
    toJSON: () => ({}),
  } as DOMRect;
}
