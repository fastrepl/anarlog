import { act, renderHook } from "@testing-library/react";
import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";

import {
  preserveScrollPosition,
  useAutoScroll,
  useScrollDetection,
} from "./viewport-hooks";

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

    act(() => {
      element.scrollTop = 900;
      element.dispatchEvent(new Event("scroll"));
    });

    expect(result.current.autoScrollEnabled).toBe(true);
    expect(result.current.scrollTarget).toBeNull();
  });
});

describe("useAutoScroll", () => {
  it("defers ongoing bottom pinning to CSS overflow-anchor once scrolled past top", () => {
    const resizeObservers: Array<{ trigger: () => void }> = [];
    const PreviousResizeObserver = globalThis.ResizeObserver;
    const previousCss = globalThis.CSS;
    const frames: FrameRequestCallback[] = [];
    const requestAnimationFrame = vi
      .spyOn(window, "requestAnimationFrame")
      .mockImplementation((callback) => {
        frames.push(callback);
        return frames.length;
      });
    globalThis.ResizeObserver = class implements ResizeObserver {
      constructor(private readonly callback: ResizeObserverCallback) {
        resizeObservers.push({ trigger: () => this.callback([], this) });
      }
      observe() {}
      unobserve() {}
      disconnect() {}
    };
    globalThis.CSS = {
      ...previousCss,
      supports: (property: string, value?: string) =>
        property === "overflow-anchor" && value === "auto",
    } as typeof CSS;

    try {
      const element = document.createElement("div");
      setScrollMetrics(element, {
        clientHeight: 100,
        scrollHeight: 1000,
        scrollTop: 500,
      });
      const containerRef = createRef<HTMLDivElement>();
      containerRef.current = element;

      renderHook(({ count }) => useAutoScroll(containerRef, [count], true), {
        initialProps: { count: 1 },
      });
      while (frames.length > 0) {
        frames.shift()?.(0);
      }
      expect(element.scrollTop).toBe(1000);

      element.scrollTop = 500;
      Object.defineProperty(element, "scrollHeight", {
        configurable: true,
        value: 1400,
      });

      act(() => {
        resizeObservers.forEach((observer) => observer.trigger());
      });
      while (frames.length > 0) {
        frames.shift()?.(0);
      }

      expect(element.scrollTop).toBe(500);
    } finally {
      globalThis.ResizeObserver = PreviousResizeObserver;
      globalThis.CSS = previousCss;
      requestAnimationFrame.mockRestore();
    }
  });
});

describe("preserveScrollPosition", () => {
  it("restores the viewport after the mutation and follow-up layout frames", async () => {
    const frames: FrameRequestCallback[] = [];
    const requestAnimationFrame = vi
      .spyOn(window, "requestAnimationFrame")
      .mockImplementation((callback) => {
        frames.push(callback);
        return frames.length;
      });
    const element = document.createElement("div");
    element.scrollTop = 420;

    await preserveScrollPosition(element, async () => {
      element.scrollTop = 0;
    });

    expect(element.scrollTop).toBe(420);

    element.scrollTop = 0;
    frames.shift()?.(0);
    expect(element.scrollTop).toBe(420);

    element.scrollTop = 0;
    frames.shift()?.(0);
    expect(element.scrollTop).toBe(420);

    element.scrollTop = 0;
    element.append("updated transcript");
    await Promise.resolve();
    expect(element.scrollTop).toBe(420);

    requestAnimationFrame.mockRestore();
  });
});
