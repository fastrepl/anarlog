import {
  getOptions,
  ReactScanDevtools,
  ReactScanInternals,
  setOptions,
  start,
  Store,
  type ScanNotification,
} from "react-scan";

import type { ReactScanSettings } from "./react-tools";
import { registerReactTools, updateReactTools } from "./react-tools";
import { publishScanData, registerScanData, type ScanEvent } from "./scan-data";

const OPTIONS_KEY = "react-scan-options";

/** Version-specific access stays here; the toolbar consumes plain summaries. */
export function installReactScan(): () => void {
  const saved = readSavedOptions();
  setOptions({
    enabled: typeof saved.enabled === "boolean" ? saved.enabled : false,
    log: saved.log === true,
    animationSpeed:
      saved.animationSpeed === "slow" || saved.animationSpeed === "off"
        ? saved.animationSpeed
        : "fast",
    showFPS: saved.showFPS !== false,
    showNotificationCount: saved.showNotificationCount !== false,
    showToolbar: false,
    // Loaded only after the native showDevtool gate permits diagnostics.
    dangerouslyForceRunInProduction: true,
    safeArea: { top: 24, right: 8, bottom: 32, left: 8 },
  });
  // scan({ enabled: false, showToolbar: false }) returns without installing.
  // start() installs collection while the floating widget stays disabled.
  start();

  const setToolbarVisible = (visible: boolean) => {
    if (!visible) Store.inspectState.value = { kind: "inspect-off" };
    updateReactTools({ toolbarVisible: visible });
  };
  Store.inspectState.value = { kind: "inspect-off" };
  let events: ScanNotification[] = [];
  const syncEvents = () => {
    events = ReactScanDevtools.getEvents();
    publishScanData({ events: events.map(summarizeEvent).reverse() });
  };
  const unregisterData = registerScanData({
    clear: () => ReactScanDevtools.clear(),
    getPrompt(id, mode) {
      const event = events.find((event) => event.id === id);
      return event ? ReactScanDevtools.getPrompt(mode, event) : "";
    },
    mountInspector: (host) => ReactScanDevtools.mountInspector(host),
  });
  syncEvents();
  const unsubscribeEvents = ReactScanDevtools.subscribe(syncEvents);
  const unregister = registerReactTools({
    setToolbarVisible,
    setOutlinesEnabled(enabled) {
      // 0.5.7 preserves the stored enabled value inside setOptions. Write it
      // first so settings changes preserve the bar's outline preference.
      try {
        localStorage.setItem(
          OPTIONS_KEY,
          JSON.stringify({ ...readSavedOptions(), enabled }),
        );
      } catch {
        // Scanning still works when browser storage is unavailable.
      }
      setOptions({ enabled });
    },
    setInspecting(inspecting) {
      if (inspecting) setToolbarVisible(true);
      Store.inspectState.value = inspecting
        ? { kind: "inspecting", hoveredDomElement: null }
        : { kind: "inspect-off" };
    },
    setSettings(settings: Partial<ReactScanSettings>) {
      setOptions(settings);
    },
    readReport: () =>
      [...Store.reportData.entries()]
        .map(([id, data]) => ({
          id,
          name: data.displayName,
          renders: data.count,
          totalTimeMs: data.time,
        }))
        .sort((a, b) => b.totalTimeMs - a.totalTimeMs)
        .slice(0, 50),
  });
  const unsubscribers = [
    getOptions().subscribe((options) =>
      updateReactTools({
        version: ReactScanInternals.version,
        settings: {
          log: options.log === true,
          animationSpeed: options.animationSpeed ?? "fast",
          showFPS: options.showFPS !== false,
          showNotificationCount: options.showNotificationCount !== false,
        },
      }),
    ),
    Store.inspectState.subscribe((state) =>
      updateReactTools({
        inspecting: state.kind === "inspecting" || state.kind === "focused",
      }),
    ),
    ReactScanInternals.instrumentation?.isPaused.subscribe((paused) =>
      updateReactTools({ outlinesEnabled: !paused }),
    ),
  ];

  return () => {
    unsubscribers.forEach((unsubscribe) => unsubscribe?.());
    unsubscribeEvents();
    events = [];
    unregisterData();
    setToolbarVisible(false);
    unregister();
  };
}

function readSavedOptions(): Record<string, unknown> {
  try {
    const value: unknown = JSON.parse(
      localStorage.getItem(OPTIONS_KEY) ?? "{}",
    );
    return value && typeof value === "object" && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

function summarizeEvent(event: ScanNotification): ScanEvent {
  const timing = event.timing;
  return {
    id: event.id,
    kind: event.kind,
    label:
      event.kind === "interaction"
        ? `${event.type === "keyboard" ? "Typed in" : "Clicked"} ${ReactScanDevtools.getComponentName(event.componentPath)}`
        : "FPS drop",
    timestamp: event.timestamp,
    duration: ReactScanDevtools.getTotalTime(timing),
    fps: event.kind === "dropped-frames" ? event.fps : null,
    severity: ReactScanDevtools.getEventSeverity(event),
    path: event.kind === "interaction" ? event.componentPath : [],
    timings:
      timing.kind === "interaction"
        ? [
            { label: "React renders", time: timing.renderTime },
            { label: "JavaScript / React hooks", time: timing.otherJSTime },
            { label: "Frame preparation", time: timing.framePreparation },
            { label: "DOM updates / layout", time: timing.frameConstruction },
            { label: "Frame presentation", time: timing.frameDraw ?? 0 },
          ]
        : [
            { label: "React renders", time: timing.renderTime },
            { label: "JavaScript / DOM / drawing", time: timing.otherTime },
          ],
    components: event.groupedFiberRenders
      .map((component) => ({
        name: component.name,
        renders: component.count,
        time: component.totalTime,
        compiled: component.hasMemoCache,
        mounted: component.wasFiberRenderMount,
        changes: [
          ...component.changes.props.map((change) => ({
            ...change,
            kind: "prop",
          })),
          ...component.changes.context.map((change) => ({
            ...change,
            kind: "context",
          })),
          ...component.changes.state.map((change) => ({
            name: String(change.index),
            count: change.count,
            kind: "state",
          })),
        ],
      }))
      .sort((a, b) => b.time - a.time),
  };
}
