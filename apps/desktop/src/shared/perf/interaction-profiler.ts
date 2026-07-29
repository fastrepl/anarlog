/**
 * Dev-only attribution for slow interactions.
 *
 * React Scan reports *how much* time an interaction spent outside React
 * rendering, but not *which* function burned it. Long Animation Frames carry
 * per-script attribution (source URL, function name, char offset), which is
 * what actually names the culprit.
 *
 * LoAF is Chromium-only, so on WebKit (macOS Tauri) this degrades to reporting
 * which keystrokes blocked and for how long; use Safari Web Inspector's
 * Timeline for attribution there.
 */

type LoafScript = PerformanceEntry & {
  duration: number;
  invoker?: string;
  invokerType?: string;
  sourceURL?: string;
  sourceFunctionName?: string;
  sourceCharPosition?: number;
  forcedStyleAndLayoutDuration?: number;
};

type LoafEntry = PerformanceEntry & {
  blockingDuration: number;
  renderStart: number;
  styleAndLayoutStart: number;
  scripts: LoafScript[];
};

const SLOW_FRAME_MS = 100;
const SLOW_KEYSTROKE_MS = 100;

function observe(type: string, handle: (entries: PerformanceEntry[]) => void) {
  try {
    const observer = new PerformanceObserver((list) =>
      handle(list.getEntries()),
    );
    observer.observe({ type, buffered: true } as PerformanceObserverInit);
    return true;
  } catch {
    return false;
  }
}

function reportLongAnimationFrames(entries: PerformanceEntry[]) {
  for (const entry of entries as LoafEntry[]) {
    if (entry.duration < SLOW_FRAME_MS) continue;

    const scripts = [...(entry.scripts ?? [])]
      .sort((left, right) => right.duration - left.duration)
      .slice(0, 8)
      .map((script) => ({
        ms: Math.round(script.duration),
        forcedLayoutMs: Math.round(script.forcedStyleAndLayoutDuration ?? 0),
        fn: script.sourceFunctionName || "(anonymous)",
        source: `${script.sourceURL ?? "?"}:${script.sourceCharPosition ?? -1}`,
        invoker: `${script.invokerType ?? "?"} ${script.invoker ?? ""}`.trim(),
      }));

    console.group(
      `[perf] long frame ${Math.round(entry.duration)}ms (blocking ${Math.round(entry.blockingDuration)}ms)`,
    );
    console.table(scripts);
    console.groupEnd();
  }
}

function reportSlowEvents(entries: PerformanceEntry[]) {
  for (const entry of entries as PerformanceEventTiming[]) {
    console.warn(
      `[perf] slow ${entry.name}: total ${Math.round(entry.duration)}ms, ` +
        `handlers ${Math.round(entry.processingEnd - entry.processingStart)}ms, ` +
        `input delay ${Math.round(entry.processingStart - entry.startTime)}ms`,
      entry.target,
    );
  }
}

/** Fallback for engines without Long Animation Frames (WebKit). */
function watchKeystrokes() {
  window.addEventListener(
    "keydown",
    (event) => {
      const start = performance.now();
      requestAnimationFrame(() => {
        const blocked = performance.now() - start;
        if (blocked < SLOW_KEYSTROKE_MS) return;
        console.warn(
          `[perf] keydown "${event.key}" blocked ${Math.round(blocked)}ms before the next frame`,
        );
      });
    },
    { capture: true },
  );
}

export function startInteractionProfiler() {
  if (!import.meta.env.DEV) return;

  const hasLoaf = observe("long-animation-frame", reportLongAnimationFrames);

  try {
    const observer = new PerformanceObserver((list) =>
      reportSlowEvents(list.getEntries()),
    );
    observer.observe({
      type: "event",
      durationThreshold: SLOW_FRAME_MS,
      buffered: true,
    } as PerformanceObserverInit);
  } catch {
    // Event Timing unavailable; the keystroke watcher still reports blocking.
  }

  watchKeystrokes();

  if (!hasLoaf) {
    console.info(
      "[perf] Long Animation Frames unsupported (WebKit). Slow keystrokes will be " +
        "reported without attribution -- run `pnpm -F @anlg/desktop dev` in Chrome, " +
        "or profile with Safari Web Inspector, to name the function.",
    );
  }
}
