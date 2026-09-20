import { useSyncExternalStore } from "react";

// Keep the runtime behind the native showDevtool gate.
export type ReactScanSettings = Readonly<{
  log: boolean;
  animationSpeed: "slow" | "fast" | "off";
  showFPS: boolean;
  showNotificationCount: boolean;
}>;

export type ReactToolsState = Readonly<{
  available: boolean;
  version: string | null;
  toolbarVisible: boolean;
  outlinesEnabled: boolean;
  inspecting: boolean;
  settings: ReactScanSettings;
}>;

export type ReactScanReport = Readonly<{
  id: number;
  name: string | null;
  renders: number;
  totalTimeMs: number;
}>;

export type ReactToolsControls = Readonly<{
  setToolbarVisible: (visible: boolean) => void;
  setOutlinesEnabled: (enabled: boolean) => void;
  setInspecting: (inspecting: boolean) => void;
  setSettings: (settings: Partial<ReactScanSettings>) => void;
  readReport: () => readonly ReactScanReport[];
}>;

const UNAVAILABLE: ReactToolsState = {
  available: false,
  version: null,
  toolbarVisible: false,
  outlinesEnabled: false,
  inspecting: false,
  settings: {
    log: false,
    animationSpeed: "fast",
    showFPS: true,
    showNotificationCount: true,
  },
};

let controls: ReactToolsControls | null = null;
let snapshot: ReactToolsState = UNAVAILABLE;
let commitCount = 0;
const listeners = new Set<() => void>();

export function registerReactTools(next: ReactToolsControls): () => void {
  controls = next;
  updateReactTools({ available: true });
  return () => {
    if (controls !== next) return;
    controls = null;
    updateReactTools(UNAVAILABLE);
  };
}

/** Bumped on every React commit; the metrics store diffs it per second. */
export function recordReactCommit(): void {
  commitCount += 1;
}

export function readReactCommitCount(): number {
  return commitCount;
}

export function setReactToolbarVisible(visible: boolean): void {
  controls?.setToolbarVisible(visible);
}

export function setReactOutlinesEnabled(enabled: boolean): void {
  controls?.setOutlinesEnabled(enabled);
}

export function setReactInspecting(inspecting: boolean): void {
  controls?.setInspecting(inspecting);
}

export function setReactScanSettings(
  settings: Partial<ReactScanSettings>,
): void {
  controls?.setSettings(settings);
}

export function readReactScanReport(): readonly ReactScanReport[] {
  return controls?.readReport() ?? [];
}

export function updateReactTools(patch: Partial<ReactToolsState>): void {
  if (
    Object.entries(patch).every(
      ([key, value]) => snapshot[key as keyof ReactToolsState] === value,
    )
  ) {
    return;
  }
  snapshot = { ...snapshot, ...patch };
  for (const listener of listeners) listener();
}

export function useReactToolsState(): ReactToolsState {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}

export function resetReactToolsForTests(): void {
  controls = null;
  commitCount = 0;
  snapshot = UNAVAILABLE;
  listeners.clear();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function getSnapshot(): ReactToolsState {
  return snapshot;
}
