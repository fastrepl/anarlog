import { cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  emit: vi.fn((_event: string, _payload: unknown) => Promise.resolve()),
  listeners: [] as Array<
    (event: {
      payload: {
        factor: number;
        sequence: number;
        source: string;
        timestamp: number;
      };
    }) => void
  >,
  setZoom: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: mocks.emit,
  listen: (
    _event: string,
    handler: (event: {
      payload: {
        factor: number;
        sequence: number;
        source: string;
        timestamp: number;
      };
    }) => void,
  ) => {
    mocks.listeners.push(handler);
    return Promise.resolve(() => {});
  },
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ label: "main", setZoom: mocks.setZoom }),
}));

import {
  DEFAULT_ZOOM_FACTOR,
  persistZoomFactor,
  readZoomFactor,
  stepZoomFactor,
  useZoomShortcuts,
  ZOOM_CHANGED_EVENT,
  ZOOM_STORAGE_KEY,
  ZOOM_STEPS,
} from "./zoom";

describe("stepZoomFactor", () => {
  it("steps in from the default", () => {
    expect(stepZoomFactor(1, "in")).toBe(1.1);
  });

  it("steps out from the default", () => {
    expect(stepZoomFactor(1, "out")).toBe(0.9);
  });

  it("clamps at the bounds", () => {
    const max = ZOOM_STEPS[ZOOM_STEPS.length - 1];
    const min = ZOOM_STEPS[0];
    expect(stepZoomFactor(max, "in")).toBe(max);
    expect(stepZoomFactor(min, "out")).toBe(min);
  });

  it("returns the default on reset", () => {
    expect(stepZoomFactor(2, "reset")).toBe(DEFAULT_ZOOM_FACTOR);
  });

  it("moves to the next step when between steps", () => {
    expect(stepZoomFactor(1.05, "in")).toBe(1.1);
    expect(stepZoomFactor(1.05, "out")).toBe(1);
  });
});

describe("readZoomFactor", () => {
  const storage = (value: string | null) => ({ getItem: vi.fn(() => value) });

  it("defaults to 1 without a stored value", () => {
    expect(readZoomFactor(storage(null))).toBe(1);
  });

  it("reads a stored factor", () => {
    expect(readZoomFactor(storage("1.25"))).toBe(1.25);
  });

  it("falls back on garbage", () => {
    expect(readZoomFactor(storage("bogus"))).toBe(1);
    expect(readZoomFactor(storage("-2"))).toBe(1);
  });
});

describe("persistZoomFactor", () => {
  it("writes the factor to storage", () => {
    const setItem = vi.fn();
    persistZoomFactor(1.5, { setItem });
    expect(setItem).toHaveBeenCalledWith(ZOOM_STORAGE_KEY, "1.5");
  });
});

describe("useZoomShortcuts", () => {
  beforeEach(() => {
    vi.stubGlobal("isTauri", true);
    mocks.emit.mockClear();
    mocks.listeners.length = 0;
    mocks.setZoom.mockClear();
    localStorage.clear();
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  const keydown = (init: KeyboardEventInit) =>
    window.dispatchEvent(
      new KeyboardEvent("keydown", { cancelable: true, ...init }),
    );

  it("applies the persisted factor on mount", () => {
    localStorage.setItem(ZOOM_STORAGE_KEY, "1.25");
    renderHook(() => useZoomShortcuts());
    expect(mocks.setZoom).toHaveBeenCalledWith(1.25);
  });

  it("zooms in with mod+= and persists and broadcasts it", () => {
    renderHook(() => useZoomShortcuts());
    keydown({ key: "=", metaKey: true });
    expect(mocks.setZoom).toHaveBeenLastCalledWith(1.1);
    expect(localStorage.getItem(ZOOM_STORAGE_KEY)).toBe("1.1");
    expect(mocks.emit).toHaveBeenCalledWith(ZOOM_CHANGED_EVENT, {
      factor: 1.1,
      source: "main",
      timestamp: expect.any(Number),
      sequence: 0,
    });
  });

  it("zooms out with ctrl+-", () => {
    renderHook(() => useZoomShortcuts());
    keydown({ key: "-", ctrlKey: true });
    expect(mocks.setZoom).toHaveBeenLastCalledWith(0.9);
  });

  it("resets with mod+0", () => {
    localStorage.setItem(ZOOM_STORAGE_KEY, "2");
    renderHook(() => useZoomShortcuts());
    keydown({ key: "0", metaKey: true });
    expect(mocks.setZoom).toHaveBeenLastCalledWith(DEFAULT_ZOOM_FACTOR);
    expect(localStorage.getItem(ZOOM_STORAGE_KEY)).toBe("1");
  });

  it("ignores keys without a modifier and alt-combos", () => {
    renderHook(() => useZoomShortcuts());
    keydown({ key: "=" });
    keydown({ key: "=", metaKey: true, altKey: true });
    expect(mocks.setZoom).toHaveBeenCalledTimes(1);
  });

  it("follows zoom changes broadcast from other windows", () => {
    renderHook(() => useZoomShortcuts());
    mocks.listeners[0]({
      payload: { factor: 1.5, source: "note", timestamp: 1, sequence: 0 },
    });
    expect(mocks.setZoom).toHaveBeenLastCalledWith(1.5);
    expect(localStorage.getItem(ZOOM_STORAGE_KEY)).toBe("1.5");
    expect(mocks.emit).not.toHaveBeenCalled();
  });

  it("ignores its own broadcast echo", () => {
    renderHook(() => useZoomShortcuts());
    keydown({ key: "=", metaKey: true });
    const calls = mocks.setZoom.mock.calls.length;
    mocks.listeners[0]({
      payload: {
        factor: 0.5,
        source: "main",
        timestamp: Date.now(),
        sequence: 0,
      },
    });
    expect(mocks.setZoom).toHaveBeenCalledTimes(calls);
    expect(localStorage.getItem(ZOOM_STORAGE_KEY)).toBe("1.1");
  });

  it("ignores stale timestamps from other windows", () => {
    renderHook(() => useZoomShortcuts());
    keydown({ key: "=", metaKey: true });
    const calls = mocks.setZoom.mock.calls.length;
    mocks.listeners[0]({
      payload: {
        factor: 0.5,
        source: "note",
        timestamp: Date.now() - 60_000,
        sequence: 0,
      },
    });
    expect(mocks.setZoom).toHaveBeenCalledTimes(calls);
  });

  it("orders rapid same-millisecond changes by sequence", () => {
    renderHook(() => useZoomShortcuts());
    keydown({ key: "=", metaKey: true });
    keydown({ key: "=", metaKey: true });
    const payloads = mocks.emit.mock.calls.map(
      (call) => call[1] as { sequence: number; timestamp: number },
    );
    expect(
      payloads[1].timestamp > payloads[0].timestamp ||
        payloads[1].sequence > payloads[0].sequence,
    ).toBe(true);
  });

  it("accepts a later real timestamp after a same-millisecond burst", () => {
    renderHook(() => useZoomShortcuts());
    keydown({ key: "=", metaKey: true });
    keydown({ key: "=", metaKey: true });
    const calls = mocks.emit.mock.calls;
    const last = calls[calls.length - 1][1] as {
      timestamp: number;
    };
    expect(last.timestamp).toBeGreaterThan(0);
    mocks.listeners[0]({
      payload: {
        factor: 0.5,
        source: "note",
        timestamp: last.timestamp + 1,
        sequence: 0,
      },
    });
    expect(localStorage.getItem(ZOOM_STORAGE_KEY)).toBe("0.5");
  });

  it("breaks equal-timestamp ties by sequence then source", () => {
    renderHook(() => useZoomShortcuts());
    mocks.listeners[0]({
      payload: { factor: 1.5, source: "note", timestamp: 1, sequence: 0 },
    });
    mocks.listeners[0]({
      payload: { factor: 0.5, source: "aaa", timestamp: 1, sequence: 0 },
    });
    expect(localStorage.getItem(ZOOM_STORAGE_KEY)).toBe("1.5");
    mocks.listeners[0]({
      payload: { factor: 1.7, source: "zzz", timestamp: 1, sequence: 0 },
    });
    expect(localStorage.getItem(ZOOM_STORAGE_KEY)).toBe("1.7");
  });

  it("stops responding after unmount", () => {
    const { unmount } = renderHook(() => useZoomShortcuts());
    unmount();
    keydown({ key: "=", metaKey: true });
    expect(mocks.setZoom).toHaveBeenCalledTimes(1);
  });
});
