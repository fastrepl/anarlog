import { act, cleanup, renderHook } from "@testing-library/react";
import {
  getOptions,
  ReactScanDevtools,
  ReactScanInternals,
  start,
  Store,
} from "react-scan";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import { installReactScan } from "./react-scan";
import {
  readReactCommitCount,
  readReactScanReport,
  resetReactToolsForTests,
  setReactInspecting,
  setReactOutlinesEnabled,
  setReactScanSettings,
  setReactToolbarVisible,
  useReactToolsState,
} from "./react-tools";

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

it("initializes hidden collection and keeps saved options without restoring a floating widget", () => {
  localStorage.setItem(
    "react-scan-options",
    JSON.stringify({
      enabled: true,
      log: true,
      animationSpeed: "slow",
      showFPS: false,
      showToolbar: true,
    }),
  );
  dispose = installReactScan();
  const { result } = renderHook(useReactToolsState);
  expect(start).toHaveBeenCalledOnce();
  expect(getOptions().peek()).toMatchObject({
    enabled: true,
    log: true,
    animationSpeed: "slow",
    showFPS: false,
    showToolbar: false,
    safeArea: { bottom: 32 },
    dangerouslyForceRunInProduction: true,
  });
  expect(result.current).toMatchObject({
    available: true,
    outlinesEnabled: true,
    toolbarVisible: false,
  });
  getOptions().peek().onCommitFinish?.();
  expect(readReactCommitCount()).toBe(1);
});

it("reflects upstream inspection and outline changes and unsubscribes on disposal", () => {
  dispose = installReactScan();
  const { result } = renderHook(useReactToolsState);
  act(() => {
    ReactScanInternals.instrumentation!.isPaused.value = false;
    Store.inspectState.value = { kind: "inspecting", hoveredDomElement: null };
  });
  expect(result.current).toMatchObject({
    outlinesEnabled: true,
    inspecting: true,
  });
  act(() => dispose?.());
  expect(result.current.available).toBe(false);
  act(() => {
    Store.inspectState.value = { kind: "inspecting", hoveredDomElement: null };
  });
  expect(result.current.inspecting).toBe(false);
});

it("opens the docked panel for inspection and exits inspection when it is hidden", () => {
  dispose = installReactScan();
  const { result } = renderHook(useReactToolsState);
  act(() => setReactInspecting(true));
  expect(getOptions().peek().showToolbar).toBe(false);
  expect(result.current).toMatchObject({
    toolbarVisible: true,
    inspecting: true,
  });
  act(() => setReactToolbarVisible(false));
  expect(result.current).toMatchObject({
    toolbarVisible: false,
    inspecting: false,
  });
});

it("persists outline preference and applies settings live", () => {
  dispose = installReactScan();
  const { result } = renderHook(useReactToolsState);
  act(() => setReactOutlinesEnabled(true));
  expect(JSON.parse(localStorage.getItem("react-scan-options")!).enabled).toBe(
    true,
  );
  expect(result.current.outlinesEnabled).toBe(true);
  act(() => setReactScanSettings({ log: true, animationSpeed: "off" }));
  expect(result.current.settings).toMatchObject({
    log: true,
    animationSpeed: "off",
  });
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

it("projects live events without DOM references and delegates each prompt to upstream", async () => {
  const { readScanData, getScanPrompt, clearScanHistory } =
    await import("./scan-data");
  const event = {
    id: "frame-1",
    kind: "dropped-frames" as const,
    timestamp: 123,
    fps: 30,
    timing: { kind: "dropped-frames" as const, renderTime: 8, otherTime: 180 },
    groupedFiberRenders: [
      {
        id: "component-1",
        name: "Card",
        count: 4,
        totalTime: 8,
        hasMemoCache: false,
        wasFiberRenderMount: false,
        elements: [document.createElement("div")],
        changes: {
          props: [{ name: "expanded", count: 1 }],
          context: [],
          state: [{ index: 0, count: 2 }],
        },
      },
    ],
  };
  vi.mocked(ReactScanDevtools.getEvents).mockReturnValue([event]);
  dispose = installReactScan();
  expect(readScanData().events[0]).toMatchObject({
    id: "frame-1",
    duration: 188,
    fps: 30,
    components: [
      {
        name: "Card",
        renders: 4,
        time: 8,
        changes: [
          { kind: "prop", name: "expanded", count: 1 },
          { kind: "state", name: "0", count: 2 },
        ],
      },
    ],
  });
  expect(JSON.stringify(readScanData())).not.toContain("elements");
  for (const mode of ["fix", "explanation", "data"] as const) {
    expect(getScanPrompt("frame-1", mode)).toBe(`${mode}:frame-1`);
    expect(ReactScanDevtools.getPrompt).toHaveBeenLastCalledWith(mode, event);
  }
  clearScanHistory();
  expect(ReactScanDevtools.clear).toHaveBeenCalledOnce();
  vi.mocked(ReactScanDevtools.getEvents).mockReturnValue([]);
  const subscriptions = vi.mocked(ReactScanDevtools.subscribe).mock.calls;
  act(() => subscriptions[subscriptions.length - 1]![0]());
  expect(readScanData().events).toEqual([]);
  expect(getScanPrompt("frame-1", "fix")).toBe("");
});

it("defaults alerts on and respects an explicit saved off preference", async () => {
  const { readScanData } = await import("./scan-data");
  dispose = installReactScan();
  expect(readScanData().alertsEnabled).toBe(true);
  dispose();
  localStorage.setItem("react-scan-notifications-audio", "false");
  dispose = installReactScan();
  expect(readScanData().alertsEnabled).toBe(false);
});
