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
  const { contentRef, handleWheel, scrollRef, updateAutoScrollState } =
    useChatAutoScroll(status);

  return (
    <div
      data-testid="scroll-area"
      onScroll={updateAutoScrollState}
      onWheel={handleWheel}
      ref={(element) => {
        scrollRef.current = element;

        if (element) {
          setScrollMetrics(element);
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

  it("does not clamp back to bottom after a slow upward wheel during streaming", () => {
    render(<TestAutoScroll />);

    const scrollArea = screen.getByTestId("scroll-area");

    scrollArea.scrollTop = 500;
    fireEvent.scroll(scrollArea);

    fireEvent.wheel(scrollArea, { deltaY: -8 });
    scrollArea.scrollTop = 492;
    fireEvent.scroll(scrollArea);

    act(() => {
      resizeObservers.forEach((observer) => observer.trigger());
    });

    expect(scrollArea.scrollTop).toBe(492);
  });
});
