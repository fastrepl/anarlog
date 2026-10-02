import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import type { ChatStatus } from "ai";
import { beforeEach, describe, expect, it } from "vitest";

import { useChatAutoScroll } from "./use-chat-auto-scroll";

const resizeObservers: MockResizeObserver[] = [];

class MockResizeObserver implements ResizeObserver {
  constructor(private readonly callback: ResizeObserverCallback) {
    resizeObservers.push(this);
  }

  observe() {}

  unobserve() {}

  disconnect() {}

  trigger() {
    this.callback([], this);
  }
}

function TestAutoScroll({ status = "streaming" }: { status?: ChatStatus }) {
  const {
    contentRef,
    handleKeyDown,
    handlePointerDown,
    handlePointerMove,
    handleWheel,
    scrollRef,
    updateAutoScrollState,
  } = useChatAutoScroll(status);

  return (
    <div
      data-testid="scroll-area"
      onKeyDown={handleKeyDown}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onScroll={updateAutoScrollState}
      onWheel={handleWheel}
      ref={(element) => {
        scrollRef.current = element;

        if (element && !element.dataset.scrollMetricsReady) {
          setScrollMetrics(element);
          element.dataset.scrollMetricsReady = "true";
        }
      }}
    >
      <div ref={contentRef} />
    </div>
  );
}

function setScrollMetrics(element: HTMLElement) {
  Object.defineProperty(element, "clientHeight", {
    configurable: true,
    value: 500,
  });
  Object.defineProperty(element, "scrollHeight", {
    configurable: true,
    value: 1000,
  });
}

describe("useChatAutoScroll", () => {
  beforeEach(() => {
    cleanup();
    resizeObservers.length = 0;
    globalThis.ResizeObserver = MockResizeObserver;
  });

  it("stays pinned when streaming content grows without user input", () => {
    render(<TestAutoScroll />);

    const scrollArea = screen.getByTestId("scroll-area");

    scrollArea.scrollTop = 500;
    fireEvent.scroll(scrollArea);
    Object.defineProperty(scrollArea, "scrollHeight", {
      configurable: true,
      value: 1400,
    });
    fireEvent.scroll(scrollArea);

    act(() => {
      resizeObservers.forEach((observer) => observer.trigger());
    });

    expect(scrollArea.scrollTop).toBe(1400);
  });

  it.each([
    [
      "wheel",
      (scrollArea: HTMLElement) => fireEvent.wheel(scrollArea, { deltaY: -8 }),
      492,
    ],
    ["scrollbar", (_scrollArea: HTMLElement) => {}, 420],
    [
      "pointer",
      (scrollArea: HTMLElement) =>
        fireEvent.pointerMove(scrollArea, {
          buttons: 1,
          pointerType: "mouse",
        }),
      420,
    ],
    [
      "keyboard",
      (scrollArea: HTMLElement) =>
        fireEvent.keyDown(scrollArea, { key: "PageUp" }),
      420,
    ],
  ])("stops following after %s scroll intent", (_, signalIntent, scrollTop) => {
    render(<TestAutoScroll />);

    const scrollArea = screen.getByTestId("scroll-area");
    scrollArea.scrollTop = 500;
    fireEvent.scroll(scrollArea);
    signalIntent(scrollArea);
    scrollArea.scrollTop = scrollTop;
    fireEvent.scroll(scrollArea);

    act(() => {
      resizeObservers.forEach((observer) => observer.trigger());
    });

    expect(scrollArea.scrollTop).toBe(scrollTop);
  });
});
