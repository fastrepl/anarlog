import { getIdentifier, getVersion } from "@tauri-apps/api/app";

import { getCloudsyncStatus } from "@anlg/plugin-db";
import { commands as miscCommands } from "@anlg/plugin-misc";

import { getTopIpcCommands, useDevtoolsMetrics } from "./metrics";
import { getTopRenderedComponents } from "./render-tracker";

import { copyText } from "~/settings/developers/clipboard";

function summarize(history: number[]) {
  if (!history.length) return null;
  const sum = history.reduce((total, value) => total + value, 0);
  return {
    last: history[history.length - 1],
    avg: Math.round(sum / history.length),
    min: Math.min(...history),
    max: Math.max(...history),
    history,
  };
}

async function attempt<T>(run: () => Promise<T>): Promise<T | null> {
  try {
    return await run();
  } catch {
    return null;
  }
}

/** Snapshot of everything the bar knows, for pasting into bug reports. */
export async function buildDiagnostics() {
  const metrics = useDevtoolsMetrics.getState();
  const identifier = await attempt(() => getIdentifier());
  const version = await attempt(() => getVersion());
  const device = await attempt(async () => {
    const result = await miscCommands.getDeviceInfo(navigator.language);
    return result.status === "ok" ? result.data : null;
  });
  const cloudsync = await attempt(() => getCloudsyncStatus());

  return {
    capturedAt: new Date().toISOString(),
    build: {
      identifier,
      version,
      hash: device?.buildHash ?? null,
      mode: import.meta.env.MODE,
    },
    device: device
      ? {
          platform: device.platform,
          arch: device.arch,
          osVersion: device.osVersion,
          totalMemoryBytes: device.totalMemoryBytes,
        }
      : null,
    window: {
      innerWidth: window.innerWidth,
      innerHeight: window.innerHeight,
      devicePixelRatio: window.devicePixelRatio,
      userAgent: navigator.userAgent,
    },
    metrics: {
      fps: summarize(metrics.fps),
      jankPercent: summarize(metrics.jank),
      delayMs: summarize(metrics.delay),
      ipcInvokes: summarize(metrics.invokes),
      ipcCallbacks: summarize(metrics.callbacks),
      httpRequests: summarize(metrics.requests),
      httpRequestsInFlight: metrics.requestsInFlight,
      renders: summarize(metrics.renders),
      memoryBytes: summarize(metrics.memoryBytes),
    },
    topIpcCommands: getTopIpcCommands(),
    topRenderedComponents: getTopRenderedComponents(),
    cloudsync,
  };
}

export async function copyDiagnostics() {
  const diagnostics = await buildDiagnostics();
  await copyText(JSON.stringify(diagnostics, null, 2), "Diagnostics copied");
}
