import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./react-scan", () => ({
  drainReactScanRenders: vi.fn(() => 0),
}));

vi.mock("@anlg/plugin-misc", () => ({
  commands: {
    getProcessMemoryBytes: vi.fn(),
  },
}));

import { commands as miscCommands } from "@anlg/plugin-misc";

import {
  formatBytes,
  HISTORY_LENGTH,
  installIpcCounters,
  isTauriIpcUrl,
  pushSample,
  startDevtoolsMetrics,
  useDevtoolsMetrics,
} from "./metrics";

const internals = () =>
  (window as unknown as { __TAURI_INTERNALS__: Record<string, unknown> })
    .__TAURI_INTERNALS__;

describe("pushSample", () => {
  it("appends and keeps only the most recent samples", () => {
    const history = Array.from({ length: HISTORY_LENGTH }, (_, i) => i);

    const next = pushSample(history, 99);

    expect(next).toHaveLength(HISTORY_LENGTH);
    expect(next[0]).toBe(1);
    expect(next[next.length - 1]).toBe(99);
    expect(history).toHaveLength(HISTORY_LENGTH);
  });

  it("grows until the limit is reached", () => {
    expect(pushSample([], 1)).toEqual([1]);
    expect(pushSample([1], 2)).toEqual([1, 2]);
  });
});

describe("formatBytes", () => {
  it("formats megabytes and gigabytes", () => {
    expect(formatBytes(36 * 1024 ** 2)).toBe("36MB");
    expect(formatBytes(1.5 * 1024 ** 3)).toBe("1.50GB");
  });
});

describe("isTauriIpcUrl", () => {
  it("matches the ipc scheme on every platform", () => {
    expect(isTauriIpcUrl("ipc://localhost/plugin%3Amisc%7Cget_git_hash")).toBe(
      true,
    );
    expect(isTauriIpcUrl("http://ipc.localhost/plugin%3Amisc")).toBe(true);
    expect(isTauriIpcUrl("https://api.anarlog.so/v1")).toBe(false);
  });
});

describe("installIpcCounters", () => {
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
    const counters = installIpcCounters();

    void fetch("ipc://localhost/plugin%3Amisc%7Cget_git_hash", {
      method: "POST",
    });
    void fetch("https://api.anarlog.so/v1");
    callbacks.set(1, () => {});
    callbacks.get(1);
    callbacks.get(2);

    expect(counters.drain()).toEqual({ invokes: 1, callbacks: 2 });
    expect(counters.drain()).toEqual({ invokes: 0, callbacks: 0 });

    counters.restore();
  });

  it("ignores traffic issued inside ignoring()", () => {
    const counters = installIpcCounters();

    counters.ignoring(() => {
      void fetch("ipc://localhost/plugin%3Amisc%7Cget_process_memory_bytes");
      callbacks.set(10, () => {});
      callbacks.set(11, () => {});
    });
    callbacks.get(10);
    callbacks.delete(11);
    callbacks.set(12, () => {});
    callbacks.get(12);

    expect(counters.drain()).toEqual({ invokes: 0, callbacks: 1 });

    counters.restore();
  });

  it("restores the original fetch and map methods", () => {
    const patchedFetch = window.fetch;
    const counters = installIpcCounters();

    expect(window.fetch).not.toBe(patchedFetch);
    expect(Object.getOwnPropertyNames(callbacks)).toContain("get");

    counters.restore();

    expect(window.fetch).toBe(patchedFetch);
    expect(Object.getOwnPropertyNames(callbacks)).not.toContain("get");
  });
});

describe("startDevtoolsMetrics", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useDevtoolsMetrics.setState({
      fps: [],
      invokes: [],
      callbacks: [],
      renders: [],
      memoryBytes: [],
    });
    vi.mocked(miscCommands.getProcessMemoryBytes).mockResolvedValue({
      status: "ok",
      data: 512 * 1024 ** 2,
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("samples once per second and polls memory", async () => {
    const stop = startDevtoolsMetrics();

    await vi.advanceTimersByTimeAsync(2000);

    const state = useDevtoolsMetrics.getState();
    expect(state.fps).toHaveLength(2);
    expect(state.invokes).toHaveLength(2);
    expect(state.callbacks).toHaveLength(2);
    expect(state.renders).toHaveLength(2);
    expect(state.memoryBytes).toEqual([512 * 1024 ** 2, 512 * 1024 ** 2]);
    expect(miscCommands.getProcessMemoryBytes).toHaveBeenCalledTimes(2);

    stop();
    await vi.advanceTimersByTimeAsync(2000);
    expect(useDevtoolsMetrics.getState().fps).toHaveLength(2);
  });
});
