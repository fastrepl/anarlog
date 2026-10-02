import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./render-tracker", () => ({
  startRenderTracker: vi.fn(() => vi.fn()),
  tickRenderTracker: vi.fn(() => 0),
}));

vi.mock("@anlg/plugin-misc", () => ({
  commands: {
    getProcessMemoryBytes: vi.fn(() => Promise.resolve({ status: "error" })),
  },
}));

import {
  getTopIpcCommands,
  installTrafficCounters,
  startDevtoolsMetrics,
  useDevtoolsMetrics,
} from "./metrics";

const internals = () =>
  (window as unknown as { __TAURI_INTERNALS__: Record<string, unknown> })
    .__TAURI_INTERNALS__;

describe("installTrafficCounters", () => {
  let originalFetch: typeof fetch;
  let callbacks: Map<number, (payload: unknown) => void>;

  beforeEach(() => {
    originalFetch = window.fetch;
    window.fetch = vi.fn().mockResolvedValue(undefined) as typeof fetch;
    callbacks = new Map();
    internals().callbacks = callbacks;
  });

  afterEach(() => {
    window.fetch = originalFetch;
    delete internals().callbacks;
  });

  it("counts ipc invokes and delivered callbacks", () => {
    const counters = installTrafficCounters();

    void fetch("ipc://localhost/plugin%3Amisc%7Cget_git_hash", {
      method: "POST",
    });
    void fetch("https://api.anarlog.so/v1");
    callbacks.set(1, () => {});
    callbacks.get(1);
    callbacks.get(2);

    expect(counters.drain()).toMatchObject({ invokes: 1, callbacks: 2 });
    expect(counters.drain()).toMatchObject({ invokes: 0, callbacks: 0 });
    expect(getTopIpcCommands()).toEqual([
      { command: "plugin:misc|get_git_hash", count: 1 },
    ]);

    counters.restore();
  });

  it("restores the original fetch and map methods", () => {
    const patchedFetch = window.fetch;
    const counters = installTrafficCounters();

    expect(window.fetch).not.toBe(patchedFetch);
    expect(Object.getOwnPropertyNames(callbacks)).toContain("get");

    counters.restore();

    expect(window.fetch).toBe(patchedFetch);
    expect(Object.getOwnPropertyNames(callbacks)).not.toContain("get");
  });

  it("clears prior metric and IPC history when collection restarts", () => {
    useDevtoolsMetrics.setState({ fps: [24] });
    const seedTraffic = installTrafficCounters();
    void fetch("ipc://localhost/plugin%3Amisc%7Cstale_command", {
      method: "POST",
    });
    seedTraffic.restore();

    const stop = startDevtoolsMetrics();
    stop();

    expect({
      metrics: useDevtoolsMetrics.getState(),
      commands: getTopIpcCommands(),
    }).toEqual({
      metrics: {
        fps: [],
        jank: [],
        delay: [],
        invokes: [],
        callbacks: [],
        requests: [],
        requestsInFlight: 0,
        renders: [],
        memoryBytes: [],
      },
      commands: [],
    });
  });
});
