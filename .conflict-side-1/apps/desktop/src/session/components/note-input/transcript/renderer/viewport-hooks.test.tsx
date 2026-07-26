import { act, renderHook } from "@testing-library/react";
import { createRef } from "react";
import { describe, expect, it } from "vitest";

import { useScrollDetection } from "./viewport-hooks";

function setScrollMetrics(
  element: HTMLDivElement,
  {
    clientHeight,
    scrollHeight,
    scrollTop,
  }: {
    clientHeight: number;
    scrollHeight: number;
    scrollTop: number;
  },
) {
  Object.defineProperty(element, "clientHeight", {
    configurable: true,
    value: clientHeight,
  });
  Object.defineProperty(element, "scrollHeight", {
    configurable: true,
    value: scrollHeight,
  });
  element.scrollTop = scrollTop;
}

describe("useScrollDetection", () => {
  it("keeps one scroll edge available when the viewport is near both edges", () => {
    const element = document.createElement("div");
    setScrollMetrics(element, {
      clientHeight: 100,
      scrollHeight: 150,
      scrollTop: 0,
    });
    const containerRef = createRef<HTMLDivElement>();
    containerRef.current = element;

    const { result } = renderHook(() =>
      useScrollDetection(containerRef, false),
    );

    expect(result.current.canScroll).toBe(true);
    expect(result.current.isAtTop).toBe(true);
    expect(result.current.isAtBottom).toBe(false);
    expect(result.current.isNearBottom).toBe(true);

    act(() => {
      element.scrollTop = 50;
      element.dispatchEvent(new Event("scroll"));
    });

    expect(result.current.isAtTop).toBe(false);
    expect(result.current.isAtBottom).toBe(true);
    expect(result.current.isNearBottom).toBe(true);
  });

  it("keeps the live transcript pinned near the exact bottom edge", () => {
    const element = document.createElement("div");
    setScrollMetrics(element, {
      clientHeight: 100,
      scrollHeight: 1000,
      scrollTop: 850,
    });
    const containerRef = createRef<HTMLDivElement>();
    containerRef.current = element;

    const { result } = renderHook(() => useScrollDetection(containerRef, true));

    expect(result.current.isAtBottom).toBe(false);
    expect(result.current.isNearBottom).toBe(true);
  });

  it("preserves manual scroll-away state when a transcript becomes active again", () => {
    const element = document.createElement("div");
    setScrollMetrics(element, {
      clientHeight: 100,
      scrollHeight: 1000,
      scrollTop: 890,
    });
    const containerRef = createRef<HTMLDivElement>();
    containerRef.current = element;

    const { result, rerender } = renderHook(
      ({ active }) => useScrollDetection(containerRef, active),
      { initialProps: { active: true } },
    );

    act(() => {
      element.scrollTop = 500;
      element.dispatchEvent(new Event("scroll"));
    });

    expect(result.current.autoScrollEnabled).toBe(false);
    expect(result.current.scrollTarget).toBe("bottom");

    rerender({ active: false });
    rerender({ active: true });

    expect(result.current.autoScrollEnabled).toBe(false);
    expect(result.current.scrollTarget).toBe("bottom");
  });
});
