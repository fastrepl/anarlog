import { create } from "zustand";

import { commands as miscCommands } from "@anlg/plugin-misc";

import { drainReactScanRenders } from "./react-scan";

export const HISTORY_LENGTH = 30;
const TICK_MS = 1000;
const MEMORY_POLL_EVERY_TICKS = 2;

export type DevtoolsMetrics = {
  fps: number[];
  invokes: number[];
  callbacks: number[];
  renders: number[];
  memoryBytes: number[];
};

export const useDevtoolsMetrics = create<DevtoolsMetrics>(() => ({
  fps: [],
  invokes: [],
  callbacks: [],
  renders: [],
  memoryBytes: [],
}));

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

export function isTauriIpcUrl(input: string): boolean {
  return /^(?:ipc:\/\/localhost|https?:\/\/ipc\.localhost)\//.test(input);
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

/**
 * Tauri freezes `__TAURI_INTERNALS__`, so IPC traffic is observed from the
 * outside: invokes go through `fetch` against the `ipc` scheme, and every
 * Rust→JS delivery (invoke responses, events, channel messages) resolves its
 * callback via `callbacks.get`, which is a plain Map we can shadow.
 *
 * Calls made while `ignoring()` runs are excluded so the bar's own memory
 * polling does not show up as traffic.
 */
export function installIpcCounters() {
  const counters = { invokes: 0, callbacks: 0 };
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
      if (ignoreDepth === 0 && isTauriIpcUrl(requestUrl(input))) {
        counters.invokes += 1;
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

export function startDevtoolsMetrics(): () => void {
  const ipc = installIpcCounters();
  let frames = 0;
  let frameHandle: number | null = null;
  if (typeof requestAnimationFrame === "function") {
    frameHandle = requestAnimationFrame(function countFrame() {
      frames += 1;
      frameHandle = requestAnimationFrame(countFrame);
    });
  }
  let lastTick = performance.now();
  let ticks = 0;
  let stopped = false;

  const sampleMemory = () => {
    void ipc
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
    const fps = Math.round(frames / elapsedSeconds);
    frames = 0;
    const traffic = ipc.drain();
    const renders = drainReactScanRenders();

    useDevtoolsMetrics.setState((state) => ({
      fps: pushSample(state.fps, fps),
      invokes: pushSample(state.invokes, traffic.invokes),
      callbacks: pushSample(state.callbacks, traffic.callbacks),
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
    if (frameHandle !== null) {
      cancelAnimationFrame(frameHandle);
    }
    ipc.restore();
  };
}
