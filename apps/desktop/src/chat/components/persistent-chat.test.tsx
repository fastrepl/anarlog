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

vi.mock("motion/react", () => ({
  AnimatePresence: ({ children }: { children: ReactNode }) => <>{children}</>,
  motion: {
    div: ({
      animate: _animate,
      children,
      exit: _exit,
      initial: _initial,
      transition: _transition,
      ...props
    }: ComponentProps<"div"> & {
      animate?: unknown;
      exit?: unknown;
      initial?: unknown;
      transition?: unknown;
    }) => <div {...props}>{children}</div>,
  },
}));

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
