import { useSyncExternalStore } from "react";

// Plain data only. The release renderer must never import React Scan's runtime.
export type PromptMode = "fix" | "explanation" | "data";
export type ScanComponent = Readonly<{
  name: string;
  renders: number;
  time: number;
  compiled: boolean;
  mounted: boolean;
  changes: readonly Readonly<{ kind: string; name: string; count: number }>[];
}>;
export type ScanEvent = Readonly<{
  id: string;
  kind: "interaction" | "dropped-frames";
  label: string;
  timestamp: number;
  duration: number;
  fps: number | null;
  severity: "low" | "needs-improvement" | "high";
  path: readonly string[];
  timings: readonly Readonly<{ label: string; time: number }>[];
  components: readonly ScanComponent[];
}>;
export type ScanData = Readonly<{
  events: readonly ScanEvent[];
  alertsEnabled: boolean;
}>;
export type ScanDataControls = Readonly<{
  clear: () => void;
  getPrompt: (id: string, mode: PromptMode) => string;
  setAlerts: (enabled: boolean) => void;
  mountInspector: (host: HTMLElement) => () => void;
}>;
const EMPTY: ScanData = { events: [], alertsEnabled: false };
let snapshot = EMPTY;
let controls: ScanDataControls | null = null;
const listeners = new Set<() => void>();

export function registerScanData(next: ScanDataControls): () => void {
  controls = next;
  return () => {
    if (controls !== next) return;
    controls = null;
    publishScanData(EMPTY);
  };
}
export function publishScanData(patch: Partial<ScanData>): void {
  snapshot = { ...snapshot, ...patch };
  for (const listener of listeners) listener();
}
export function readScanData(): ScanData {
  return snapshot;
}
export function useScanData(): ScanData {
  return useSyncExternalStore(subscribe, readScanData, readScanData);
}
function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
export function clearScanHistory(): void {
  controls?.clear();
}
export function getScanPrompt(id: string, mode: PromptMode): string {
  return controls?.getPrompt(id, mode) ?? "";
}
export function setScanAlerts(enabled: boolean): void {
  controls?.setAlerts(enabled);
}
export function mountScanInspector(host: HTMLElement): () => void {
  return controls?.mountInspector(host) ?? (() => {});
}
