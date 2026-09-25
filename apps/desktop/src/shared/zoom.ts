import { isTauri } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  currentMonitor,
  getCurrentWindow,
  LogicalSize,
} from "@tauri-apps/api/window";

import { useMountEffect } from "~/shared/hooks/useMountEffect";

export const ZOOM_STORAGE_KEY = "anarlog-zoom-factor";
export const ZOOM_CHANGED_EVENT = "anlg:zoom-factor-changed";

export const ZOOM_STEPS = [
  0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3,
] as const;

export const DEFAULT_ZOOM_FACTOR = 1;
export const ZOOM_CSS_VARIABLE = "--anlg-zoom";

// Logical (unzoomed) minimum inner sizes, mirroring plugins/windows v1.rs.
const MAIN_WINDOW_BASE_MIN_SIZE = { width: 500, height: 500 };
const NOTE_WINDOW_BASE_MIN_SIZE = { width: 420, height: 500 };

export function getWindowBaseMinSize(label: string) {
  if (label === "main") {
    return MAIN_WINDOW_BASE_MIN_SIZE;
  }
  if (label.startsWith("note-")) {
    return NOTE_WINDOW_BASE_MIN_SIZE;
  }
  return null;
}

export function scaleWindowMinSize(
  base: { width: number; height: number },
  factor: number,
  bounds: { width: number; height: number } | null,
) {
  const width = Math.round(base.width * factor);
  const height = Math.round(base.height * factor);
  if (!bounds) {
    return { width, height };
  }
  return {
    width: Math.min(width, Math.floor(bounds.width)),
    height: Math.min(height, Math.floor(bounds.height)),
  };
}

let minSizeSyncGeneration = 0;

async function syncWindowMinSize(factor: number) {
  const appWindow = getCurrentWindow();
  const base = getWindowBaseMinSize(appWindow.label);
  if (!base) {
    return;
  }

  const generation = ++minSizeSyncGeneration;
  const isStale = () => generation !== minSizeSyncGeneration;

  const [monitor, scaleFactor] = await Promise.all([
    currentMonitor(),
    appWindow.scaleFactor(),
  ]);
  if (isStale()) {
    return;
  }
  const workArea = monitor
    ? monitor.workArea.size.toLogical(monitor.scaleFactor)
    : null;
  const min = scaleWindowMinSize(base, factor, workArea);
  await appWindow.setMinSize(new LogicalSize(min.width, min.height));
  if (isStale()) {
    return;
  }

  const current = (await appWindow.innerSize()).toLogical(scaleFactor);
  if (isStale()) {
    return;
  }
  if (current.width < min.width || current.height < min.height) {
    await appWindow.setSize(
      new LogicalSize(
        Math.max(current.width, min.width),
        Math.max(current.height, min.height),
      ),
    );
  }
}

export function stepZoomFactor(
  current: number,
  direction: "in" | "out" | "reset",
): number {
  if (direction === "reset") {
    return DEFAULT_ZOOM_FACTOR;
  }

  const steps = direction === "in" ? ZOOM_STEPS : [...ZOOM_STEPS].reverse();
  const candidate = steps.find((step) =>
    direction === "in" ? step > current + 1e-6 : step < current - 1e-6,
  );

  if (candidate !== undefined) {
    return candidate;
  }

  return direction === "in" ? ZOOM_STEPS[ZOOM_STEPS.length - 1] : ZOOM_STEPS[0];
}

export function readZoomFactor(
  storage: Pick<Storage, "getItem"> = window.localStorage,
): number {
  const raw = storage.getItem(ZOOM_STORAGE_KEY);
  const factor = raw === null ? NaN : Number(raw);
  return Number.isFinite(factor) && factor > 0 ? factor : DEFAULT_ZOOM_FACTOR;
}

export function persistZoomFactor(
  factor: number,
  storage: Pick<Storage, "setItem"> = window.localStorage,
) {
  storage.setItem(ZOOM_STORAGE_KEY, String(factor));
}

export function applyZoomFactor(factor: number): Promise<void> {
  document.documentElement.style.setProperty(ZOOM_CSS_VARIABLE, String(factor));

  if (!isTauri()) {
    return Promise.resolve();
  }

  try {
    return getCurrentWebview()
      .setZoom(factor)
      .then(() => syncWindowMinSize(factor))
      .catch((error: unknown) => {
        console.warn("[zoom] failed to set webview zoom", error);
      });
  } catch (error) {
    console.warn("[zoom] failed to set webview zoom", error);
    return Promise.resolve();
  }
}

export function useZoomShortcuts() {
  useMountEffect(() => {
    const tauri = isTauri();
    const ownLabel = tauri ? getCurrentWebview().label : "";
    let lastTimestamp = 0;
    let lastSequence = 0;
    let lastSource = "";

    let factor = readZoomFactor();
    void applyZoomFactor(factor);

    const setFactor = (next: number, broadcast: boolean) => {
      factor = next;
      persistZoomFactor(factor);
      void applyZoomFactor(factor);

      if (broadcast && tauri) {
        const now = Date.now();
        if (now <= lastTimestamp) {
          lastSequence += 1;
        } else {
          lastTimestamp = now;
          lastSequence = 0;
        }
        lastSource = ownLabel;
        void emit(ZOOM_CHANGED_EVENT, {
          factor,
          source: ownLabel,
          timestamp: lastTimestamp,
          sequence: lastSequence,
        }).catch((error: unknown) => {
          console.warn("[zoom] failed to broadcast zoom factor", error);
        });
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.altKey) {
        return;
      }

      switch (event.key) {
        case "-":
        case "_":
          event.preventDefault();
          setFactor(stepZoomFactor(factor, "out"), true);
          return;
        case "=":
        case "+":
          event.preventDefault();
          setFactor(stepZoomFactor(factor, "in"), true);
          return;
        case "0":
          event.preventDefault();
          setFactor(stepZoomFactor(factor, "reset"), true);
          return;
      }
    };

    let unlisten: (() => void) | undefined;
    let cancelled = false;
    if (tauri) {
      listen<{
        factor: number;
        source: string;
        timestamp: number;
        sequence: number;
      }>(ZOOM_CHANGED_EVENT, (event) => {
        const payload = event.payload;
        if (
          !payload ||
          payload.source === ownLabel ||
          typeof payload.factor !== "number" ||
          !Number.isFinite(payload.factor) ||
          payload.factor <= 0 ||
          typeof payload.timestamp !== "number" ||
          typeof payload.sequence !== "number"
        ) {
          return;
        }
        const newer =
          payload.timestamp > lastTimestamp ||
          (payload.timestamp === lastTimestamp &&
            (payload.sequence > lastSequence ||
              (payload.sequence === lastSequence &&
                payload.source > lastSource)));
        if (!newer) {
          return;
        }
        lastTimestamp = payload.timestamp;
        lastSequence = payload.sequence;
        lastSource = payload.source;
        if (payload.factor !== factor) {
          setFactor(payload.factor, false);
        }
      })
        .then((fn) => {
          if (cancelled) {
            fn();
          } else {
            unlisten = fn;
          }
        })
        .catch((error: unknown) => {
          console.warn("[zoom] failed to subscribe to zoom changes", error);
        });
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      cancelled = true;
      unlisten?.();
      window.removeEventListener("keydown", handleKeyDown);
    };
  });
}
