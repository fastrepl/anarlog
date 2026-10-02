import { cleanup } from "@testing-library/react";
import { getOptions, ReactScanDevtools, Store } from "react-scan";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import { installReactScan } from "./react-scan";
import { readReactScanReport, resetReactToolsForTests } from "./react-tools";

vi.mock("react-scan", () => {
  // Scan's browser-only bundle imports JSON without Node import attributes.
  // Model its exported signals here; exercise the real widget in Electron.
  function signal<T>(initial: T) {
    let value = initial;
    const listeners = new Set<(next: T) => void>();
    return {
      get value() {
        return value;
      },
      set value(next: T) {
        value = next;
        listeners.forEach((listener) => listener(next));
      },
      peek: () => value,
      subscribe(listener: (next: T) => void) {
        listeners.add(listener);
        listener(value);
        return () => listeners.delete(listener);
      },
    };
  }
  const options = signal<import("react-scan").Options>({});
  const paused = signal(true);
  return {
    ReactScanDevtools: {
      getEvents: vi.fn(() => []),
      subscribe: vi.fn(() => () => {}),
      clear: vi.fn(),
      getPrompt: vi.fn((mode, event) => `${mode}:${event.id}`),
      getTotalTime: vi.fn((timing) => timing.renderTime + timing.otherTime),
      getComponentName: vi.fn((path) => path[path.length - 1]),
      getEventSeverity: vi.fn(() => "high"),
      mountInspector: vi.fn(() => () => {}),
      playNotificationSound: vi.fn(),
    },
    getOptions: () => options,
    ReactScanInternals: {
      version: "0.5.7",
      instrumentation: { isPaused: paused },
    },
    Store: {
      inspectState: signal<typeof Store.inspectState.value>({
        kind: "uninitialized",
      }),
      reportData: new Map(),
    },
    start: vi.fn(() => {
      paused.value = !options.peek().enabled;
    }),
    setOptions: vi.fn((patch) => {
      if ("enabled" in patch) paused.value = !patch.enabled;
      options.value = { ...options.peek(), ...patch };
      // Match the upstream preference quirk this adapter compensates for.
      let saved;
      try {
        saved = JSON.parse(localStorage.getItem("react-scan-options") ?? "{}");
      } catch {}
      if (typeof saved?.enabled === "boolean")
        options.peek().enabled = saved.enabled;
    }),
  };
});

let dispose: (() => void) | undefined;
beforeEach(() => {
  localStorage.clear();
  resetReactToolsForTests();
  Store.reportData.clear();
  Store.inspectState.value = { kind: "uninitialized" };
  vi.clearAllMocks();
  vi.mocked(ReactScanDevtools.getEvents).mockReturnValue([]);
});
afterEach(() => {
  cleanup();
  dispose?.();
});

it("exports bounded plain render summaries without Fibers or component values", () => {
  dispose = installReactScan();
  for (let id = 0; id < 60; id++) {
    Store.reportData.set(id, {
      count: id + 1,
      time: id * 2,
      displayName: `Component${id}`,
      renders: [],
      type: { privateValue: "must not serialize" },
    });
  }
  const report = readReactScanReport();
  expect(report).toHaveLength(50);
  expect(report[0]).toEqual({
    id: 59,
    name: "Component59",
    renders: 60,
    totalTimeMs: 118,
  });
  expect(JSON.stringify(report)).not.toContain("privateValue");
});

it("recovers from malformed saved settings", () => {
  localStorage.setItem("react-scan-options", "{invalid");
  dispose = installReactScan();
  expect(getOptions().peek()).toMatchObject({
    enabled: false,
    log: false,
    animationSpeed: "fast",
  });
});
