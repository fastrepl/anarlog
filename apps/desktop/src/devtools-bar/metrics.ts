import { create } from "zustand";

import { commands as miscCommands } from "@anlg/plugin-misc";

import { startRenderTracker, tickRenderTracker } from "./render-tracker";

export const HISTORY_LENGTH = 30;
const TICK_MS = 1000;
const MEMORY_POLL_EVERY_TICKS = 2;
const DELAY_PROBE_MS = 50;
// A frame taking longer than this dropped at least one frame at 60Hz.
const LONG_FRAME_MS = 34;
const TOP_COMMANDS_WINDOW_TICKS = 10;

export type DevtoolsMetrics = {
  fps: number[];
  /** Share of frames in each second that ran longer than a frame budget, 0–100. */
  jank: number[];
  /** Worst main-thread stall observed in each second, in ms. */
  delay: number[];
  invokes: number[];
  callbacks: number[];
  /** HTTP requests started in each second (IPC excluded). */
  requests: number[];
  requestsInFlight: number;
  renders: number[];
  memoryBytes: number[];
};

const EMPTY_METRICS: DevtoolsMetrics = {
  fps: [],
  jank: [],
  delay: [],
  invokes: [],
  callbacks: [],
  requests: [],
  requestsInFlight: 0,
  renders: [],
  memoryBytes: [],
};

export const useDevtoolsMetrics = create<DevtoolsMetrics>(() => EMPTY_METRICS);

export function resetDevtoolsMetrics() {
  useDevtoolsMetrics.setState(EMPTY_METRICS);
}

export function pushSample(
  history: number[],
  value: number,
  limit = HISTORY_LENGTH,
): number[] {
  const next = history.slice(Math.max(0, history.length - limit + 1));
  next.push(value);
  return next;
}

export function formatBytes(bytes: number): string {
  if (bytes >= 1024 ** 3) {
    return `${(bytes / 1024 ** 3).toFixed(2)}GB`;
  }
  return `${Math.round(bytes / 1024 ** 2)}MB`;
}

const IPC_URL = /^(?:ipc:\/\/localhost|https?:\/\/ipc\.localhost)\/([^?#]*)/;

export function isTauriIpcUrl(input: string): boolean {
  return IPC_URL.test(input);
}

/** `ipc://localhost/plugin%3Adb%7Cexecute` → `plugin:db|execute`. */
export function ipcCommandFromUrl(input: string): string | null {
  const match = IPC_URL.exec(input);
  if (!match) return null;
  try {
    return decodeURIComponent(match[1]!);
  } catch {
    return match[1]!;
  }
}

function isHttpUrl(input: string): boolean {
  return /^https?:\/\//.test(input) && !isTauriIpcUrl(input);
}

function requestUrl(input: RequestInfo | URL): string {
  if (typeof input === "string") return input;
  if (input instanceof URL) return input.href;
  return input.url;
}

type TauriCallbacks = Map<number, unknown>;

function getTauriCallbacks(): TauriCallbacks | null {
  const internals = (window as unknown as Record<string, unknown>)
    .__TAURI_INTERNALS__ as { callbacks?: unknown } | undefined;
  return internals?.callbacks instanceof Map
    ? (internals.callbacks as TauriCallbacks)
    : null;
}

let commandBuckets: Array<Map<string, number>> = [new Map()];

function rotateCommandBuckets() {
  commandBuckets.push(new Map());
  if (commandBuckets.length > TOP_COMMANDS_WINDOW_TICKS) {
    commandBuckets = commandBuckets.slice(
      commandBuckets.length - TOP_COMMANDS_WINDOW_TICKS,
    );
  }
}

/** Most invoked Tauri commands over the last ~10 seconds. */
export function getTopIpcCommands(
  limit = 8,
): Array<{ command: string; count: number }> {
  const totals = new Map<string, number>();
  for (const bucket of commandBuckets) {
    for (const [command, count] of bucket) {
      totals.set(command, (totals.get(command) ?? 0) + count);
    }
  }
  return [...totals]
    .map(([command, count]) => ({ command, count }))
    .sort((left, right) => right.count - left.count)
    .slice(0, limit);
}

/**
 * Tauri freezes `__TAURI_INTERNALS__`, so IPC traffic is observed from the
 * outside: invokes go through `fetch` against the `ipc` scheme, and every
 * Rust→JS delivery (invoke responses, events, channel messages) resolves its
 * callback via `callbacks.get`, which is a plain Map we can shadow. Plain
 * HTTP requests through the same `fetch` wrapper feed the Net counter.
 *
 * Calls made while `ignoring()` runs are excluded so the bar's own memory
 * polling does not show up as traffic.
 */
export function installTrafficCounters() {
  const counters = { invokes: 0, callbacks: 0, requests: 0, inFlight: 0 };
  const ignoredIds = new Set<number>();
  let ignoreDepth = 0;
  const cleanups: Array<() => void> = [];

  const originalFetch = window.fetch;
  if (typeof originalFetch === "function") {
    window.fetch = function countingFetch(
      this: unknown,
      input: RequestInfo | URL,
      init?: RequestInit,
    ) {
      const url = requestUrl(input);
      if (ignoreDepth === 0) {
        const command = ipcCommandFromUrl(url);
        if (command !== null) {
          counters.invokes += 1;
          const bucket = commandBuckets[commandBuckets.length - 1]!;
          bucket.set(command, (bucket.get(command) ?? 0) + 1);
        } else if (isHttpUrl(url)) {
          counters.requests += 1;
          counters.inFlight += 1;
          const settle = () => {
            counters.inFlight = Math.max(0, counters.inFlight - 1);
          };
          const response = originalFetch.call(this, input, init);
          response.then(settle, settle);
          return response;
        }
      }
      return originalFetch.call(this, input, init);
    } as typeof fetch;
    cleanups.push(() => {
      window.fetch = originalFetch;
    });
  }

  const callbacks = getTauriCallbacks();
  if (callbacks) {
    Object.defineProperty(callbacks, "set", {
      configurable: true,
      value(this: TauriCallbacks, id: number, callback: unknown) {
        if (ignoreDepth > 0) {
          ignoredIds.add(id);
        }
        return Map.prototype.set.call(this, id, callback);
      },
    });
    Object.defineProperty(callbacks, "get", {
      configurable: true,
      value(this: TauriCallbacks, id: number) {
        if (!ignoredIds.delete(id)) {
          counters.callbacks += 1;
        }
        return Map.prototype.get.call(this, id);
      },
    });
    Object.defineProperty(callbacks, "delete", {
      configurable: true,
      value(this: TauriCallbacks, id: number) {
        ignoredIds.delete(id);
        return Map.prototype.delete.call(this, id);
      },
    });
    cleanups.push(() => {
      delete (callbacks as Partial<TauriCallbacks>).set;
      delete (callbacks as Partial<TauriCallbacks>).get;
      delete (callbacks as Partial<TauriCallbacks>).delete;
    });
  }

  return {
    drain() {
      const snapshot = { ...counters };
      counters.invokes = 0;
      counters.callbacks = 0;
      counters.requests = 0;
      rotateCommandBuckets();
      return snapshot;
    },
    ignoring<T>(run: () => T): T {
      ignoreDepth += 1;
      try {
        return run();
      } finally {
        ignoreDepth -= 1;
      }
    },
    restore() {
      cleanups.forEach((cleanup) => cleanup());
    },
  };
}

/**
 * Frame timing from a rAF loop and main-thread stalls from a short timer's
 * drift (a 50ms timer firing 300ms late means the thread was blocked ~250ms).
 * Both pause while the window is hidden, so hidden time is not counted.
 */
function startFrameProbe() {
  let frames = 0;
  let longFrames = 0;
  let worstDelay = 0;
  let lastFrame: number | null = null;
  let frameHandle: number | null = null;

  if (typeof requestAnimationFrame === "function") {
    frameHandle = requestAnimationFrame(function onFrame(timestamp) {
      if (lastFrame !== null) {
        frames += 1;
        if (timestamp - lastFrame > LONG_FRAME_MS) longFrames += 1;
      }
      lastFrame = timestamp;
      frameHandle = requestAnimationFrame(onFrame);
    });
  }

  let expected = performance.now() + DELAY_PROBE_MS;
  const probe = setInterval(() => {
    const now = performance.now();
    if (typeof document === "undefined" || !document.hidden) {
      worstDelay = Math.max(worstDelay, now - expected);
    }
    expected = now + DELAY_PROBE_MS;
  }, DELAY_PROBE_MS);

  return {
    drain(elapsedSeconds: number) {
      const sample = {
        fps: Math.round(frames / elapsedSeconds),
        jank: frames ? Math.round((longFrames / frames) * 100) : 0,
        delay: Math.max(0, Math.round(worstDelay)),
      };
      frames = 0;
      longFrames = 0;
      worstDelay = 0;
      return sample;
    },
    stop() {
      clearInterval(probe);
      if (frameHandle !== null) cancelAnimationFrame(frameHandle);
    },
  };
}

export function startDevtoolsMetrics(): () => void {
  const traffic = installTrafficCounters();
  const frameProbe = startFrameProbe();
  const stopRenderTracker = startRenderTracker();
  let lastTick = performance.now();
  let ticks = 0;
  let stopped = false;

  const sampleMemory = () => {
    void traffic
      .ignoring(() => miscCommands.getProcessMemoryBytes())
      .then((result) => {
        if (stopped || result.status !== "ok") return;
        useDevtoolsMetrics.setState((state) => ({
          memoryBytes: pushSample(state.memoryBytes, result.data),
        }));
      })
      .catch(() => {});
  };

  const interval = setInterval(() => {
    const now = performance.now();
    const elapsedSeconds = Math.max((now - lastTick) / 1000, 0.001);
    lastTick = now;
    const frame = frameProbe.drain(elapsedSeconds);
    const io = traffic.drain();
    const renders = tickRenderTracker();

    useDevtoolsMetrics.setState((state) => ({
      fps: pushSample(state.fps, frame.fps),
      jank: pushSample(state.jank, frame.jank),
      delay: pushSample(state.delay, frame.delay),
      invokes: pushSample(state.invokes, io.invokes),
      callbacks: pushSample(state.callbacks, io.callbacks),
      requests: pushSample(state.requests, io.requests),
      requestsInFlight: io.inFlight,
      renders: pushSample(state.renders, renders),
    }));

    ticks += 1;
    if (ticks % MEMORY_POLL_EVERY_TICKS === 0) {
      sampleMemory();
    }
  }, TICK_MS);

  sampleMemory();

  return () => {
    stopped = true;
    clearInterval(interval);
    frameProbe.stop();
    stopRenderTracker();
    traffic.restore();
  };
}
