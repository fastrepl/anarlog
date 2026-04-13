import { act, render } from "@testing-library/react";
import { useRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAutoScroll } from "./viewport-hooks";

let resizeCallback: ResizeObserverCallback | null = null;

class MockResizeObserver implements ResizeObserver {
  readonly root = null;
  readonly rootMargin = "";
  readonly thresholds = [];

  constructor(callback: ResizeObserverCallback) {
    resizeCallback = callback;
  }

  disconnect() {}
  observe() {}
  unobserve() {}
}

function TestHarness({
  version,
  enabled = true,
}: {
  version: number;
  enabled?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useAutoScroll(ref, [version], enabled);

  return <div ref={ref} />;
}

function setScrollMetrics(
  element: HTMLDivElement,
  metrics: {
    clientHeight: number;
    scrollHeight: number;
    scrollTop: number;
  },
) {
  Object.defineProperty(element, "clientHeight", {
    configurable: true,
    value: metrics.clientHeight,
  });
  Object.defineProperty(element, "scrollHeight", {
    configurable: true,
    value: metrics.scrollHeight,
  });
  Object.defineProperty(element, "scrollTop", {
    configurable: true,
    value: metrics.scrollTop,
    writable: true,
  });
}

function triggerResize() {
  if (!resizeCallback) {
    throw new Error("ResizeObserver callback was not registered");
  }

  const callback = resizeCallback;
  act(() => {
    callback([], {} as ResizeObserver);
  });
}

describe("useAutoScroll", () => {
  beforeEach(() => {
    let frameId = 0;

    vi.stubGlobal(
      "requestAnimationFrame",
      ((callback: FrameRequestCallback) => {
        callback(0);
        frameId += 1;
        return frameId;
      }) satisfies typeof requestAnimationFrame,
    );
    vi.stubGlobal(
      "cancelAnimationFrame",
      vi.fn() satisfies typeof cancelAnimationFrame,
    );
    vi.stubGlobal("ResizeObserver", MockResizeObserver);
    resizeCallback = null;
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("keeps following when content grows after the viewport was pinned", () => {
    const { container, rerender } = render(<TestHarness version={0} />);
    const element = container.firstElementChild as HTMLDivElement;

    setScrollMetrics(element, {
      clientHeight: 100,
      scrollHeight: 200,
      scrollTop: 100,
    });

    rerender(<TestHarness version={1} />);

    setScrollMetrics(element, {
      clientHeight: 100,
      scrollHeight: 340,
      scrollTop: 100,
    });
    triggerResize();

    expect(element.scrollTop).toBe(340);
  });

  it("does not force-scroll when the user moved away from the bottom", () => {
    const { container, rerender } = render(<TestHarness version={0} />);
    const element = container.firstElementChild as HTMLDivElement;

    setScrollMetrics(element, {
      clientHeight: 100,
      scrollHeight: 200,
      scrollTop: 100,
    });

    rerender(<TestHarness version={1} />);

    setScrollMetrics(element, {
      clientHeight: 100,
      scrollHeight: 340,
      scrollTop: 20,
    });
    triggerResize();

    expect(element.scrollTop).toBe(20);
  });
});
