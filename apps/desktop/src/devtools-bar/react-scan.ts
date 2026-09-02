type ReactScanModule = typeof import("react-scan");

let reactScan: ReactScanModule | null = null;
let pendingRenders = 0;
const availabilityListeners = new Set<() => void>();

/**
 * React Scan piggybacks on the devtools hook that React Refresh installs, so it
 * only works under the Vite dev server. Production bundles (staging included)
 * evaluate React before any hook exists, and React Scan cannot attach late.
 */
export async function startReactScanInDev(): Promise<void> {
  if (!import.meta.env.DEV || reactScan) {
    return;
  }

  try {
    const module = await import("react-scan");
    module.scan({
      enabled: true,
      showToolbar: false,
      onRender: (_fiber, renders) => {
        pendingRenders += renders.length;
      },
    });
    reactScan = module;
    availabilityListeners.forEach((listener) => listener());
  } catch (error) {
    console.warn("Failed to start React Scan:", error);
  }
}

export function isReactScanAvailable(): boolean {
  return reactScan !== null;
}

export function subscribeReactScanAvailability(listener: () => void) {
  availabilityListeners.add(listener);
  return () => {
    availabilityListeners.delete(listener);
  };
}

/** Returns renders observed since the previous call and resets the counter. */
export function drainReactScanRenders(): number {
  const renders = pendingRenders;
  pendingRenders = 0;
  return renders;
}

/**
 * Call with a component's props object during render to keep that component
 * out of React Scan outlines and render counts. The bar re-renders every
 * second, so without this it would outline and count itself.
 */
export function ignoreReactScan(props: object): void {
  reactScan?.ignoredProps.add(props);
}

export function areReactScanOutlinesEnabled(): boolean {
  const instrumentation = reactScan?.ReactScanInternals.instrumentation;
  return instrumentation ? !instrumentation.isPaused.value : false;
}

// React Scan skips its onRender hook entirely while outlines are paused, so
// the render counter only advances when outlines are on.
export function setReactScanOutlinesEnabled(enabled: boolean): void {
  const instrumentation = reactScan?.ReactScanInternals.instrumentation;
  if (instrumentation) {
    instrumentation.isPaused.value = !enabled;
  }
}

export function isReactScanToolbarVisible(): boolean {
  return reactScan?.getOptions().value.showToolbar === true;
}

export function setReactScanToolbarVisible(visible: boolean): void {
  reactScan?.setOptions({ showToolbar: visible });
}
