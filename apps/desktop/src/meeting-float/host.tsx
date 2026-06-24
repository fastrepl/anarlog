import {
  commands as windowsCommands,
  events as windowsEvents,
  type FloatingBarSettingsChange,
} from "@hypr/plugin-windows";
import type { GeneralStorage } from "@hypr/store";

import { useConfigValue } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import * as settingsStore from "~/store/tinybase/store/settings";
import { listenerStore } from "~/store/zustand/listener/instance";

type ListenerState = ReturnType<typeof listenerStore.getState>;
type SettingsStore = NonNullable<ReturnType<typeof settingsStore.UI.useStore>>;
type FloatingBarStatus = "recording" | "error";
type FloatingBarColorScheme = "light" | "dark";
type LiveCaptionPosition =
  | "topCenter"
  | "topLeft"
  | "topRight"
  | "bottomLeft"
  | "bottomRight"
  | "bottomCenter";
type FloatingOverlaySettings = {
  floatingBarOpacity: number;
  liveCaptionOpacity: number;
  liveCaptionPosition: LiveCaptionPosition;
  liveCaptionMinimized: boolean;
};
type FloatingOverlaySettingsStorage = Pick<
  GeneralStorage,
  | "floating_bar_opacity"
  | "live_caption_opacity"
  | "live_caption_position"
  | "live_caption_minimized"
>;
type FloatingRouteState = {
  sessionId: string;
  amplitude: number;
  status: FloatingBarStatus;
  colorScheme: FloatingBarColorScheme;
  opacity: number;
  liveCaptionOpacity: number;
  liveCaptionPosition: LiveCaptionPosition;
  liveCaptionMinimized: boolean;
};
type LiveCaptionRouteState = {
  sessionId: string;
  text: string;
  opacity: number;
  position: LiveCaptionPosition;
  minimized: boolean;
};

const DEFAULT_FLOATING_OVERLAY_SETTINGS: FloatingOverlaySettings = {
  floatingBarOpacity: 0.78,
  liveCaptionOpacity: 0.78,
  liveCaptionPosition: "topCenter",
  liveCaptionMinimized: false,
};

const LIVE_CAPTION_POSITIONS: ReadonlySet<string> = new Set([
  "topCenter",
  "topLeft",
  "topRight",
  "bottomLeft",
  "bottomRight",
  "bottomCenter",
]);

const FLOATING_OVERLAY_SETTING_KEYS = [
  "floating_bar_opacity",
  "live_caption_opacity",
  "live_caption_position",
  "live_caption_minimized",
] as const;

export function FloatingMeetingWindowHost() {
  const floatingBarEnabled = useConfigValue("floating_bar_enabled");
  const store = settingsStore.UI.useStore(settingsStore.STORE_ID);

  return (
    <>
      <FloatingOverlaySettingsEventSync />
      {floatingBarEnabled ? (
        <FloatingMeetingWindowSync store={store} />
      ) : (
        <FloatingMeetingWindowDisabled />
      )}
      <LiveCaptionWindowSync store={store} />
    </>
  );
}

function getFloatingOverlaySettingsFromStore(
  store: SettingsStore | undefined,
): FloatingOverlaySettings {
  return {
    floatingBarOpacity: normalizeOpacity(
      store?.getValue("floating_bar_opacity"),
      DEFAULT_FLOATING_OVERLAY_SETTINGS.floatingBarOpacity,
    ),
    liveCaptionOpacity: normalizeOpacity(
      store?.getValue("live_caption_opacity"),
      DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionOpacity,
    ),
    liveCaptionPosition: normalizeLiveCaptionPosition(
      store?.getValue("live_caption_position"),
    ),
    liveCaptionMinimized: store?.getValue("live_caption_minimized") === true,
  };
}

function FloatingOverlaySettingsEventSync() {
  const setPartialValues = settingsStore.UI.useSetPartialValuesCallback(
    (values: Partial<FloatingOverlaySettingsStorage>) => values,
    [],
    settingsStore.STORE_ID,
  );

  useMountEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    windowsEvents.floatingBarSettingsChange
      .listen((event) => {
        if (cancelled) {
          return;
        }

        const values = getSettingsValuesFromNativeChange(event.payload);
        if (Object.keys(values).length === 0) {
          return;
        }

        setPartialValues(values);
      })
      .then((nextUnlisten) => {
        if (cancelled) {
          nextUnlisten();
          return;
        }

        unlisten = nextUnlisten;
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  });

  return null;
}

function FloatingMeetingWindowDisabled() {
  useMountEffect(() => {
    void hideFloatingMeetingPanel();
  });

  return null;
}

function LiveCaptionWindowSync({
  store,
}: {
  store: SettingsStore | undefined;
}) {
  useMountEffect(() => {
    let settings = getFloatingOverlaySettingsFromStore(store);
    let routeState = getCurrentLiveCaptionRouteState(
      listenerStore.getState(),
      settings,
    );
    let syncQueued = false;
    let syncRunning = false;
    let syncRequested = false;
    let cancelled = false;
    let shownSessionId: string | null = null;

    const shouldContinue = () => !cancelled;

    const sync = async () => {
      if (!shouldContinue()) {
        return;
      }

      const nextShownSessionId = await syncLiveCaptionWindow(
        routeState,
        shownSessionId,
        shouldContinue,
      );
      if (!shouldContinue()) {
        await hideLiveCaptionPanel();
        return;
      }

      if (nextShownSessionId === "unavailable") {
        return;
      }

      shownSessionId = nextShownSessionId;
    };

    const runQueuedSync = async () => {
      if (syncRunning) {
        syncRequested = true;
        return;
      }

      syncRunning = true;
      try {
        do {
          syncRequested = false;
          await sync();
        } while (syncRequested && !cancelled);
      } finally {
        syncRunning = false;
      }
    };

    const scheduleSync = () => {
      if (syncQueued) {
        return;
      }

      syncQueued = true;
      queueMicrotask(() => {
        syncQueued = false;
        if (cancelled) {
          return;
        }

        void runQueuedSync();
      });
    };

    scheduleSync();

    const unsubscribe = listenerStore.subscribe((state, previousState) => {
      const nextRouteState = getLiveCaptionRouteState(state, settings);
      const previousRouteState = getLiveCaptionRouteState(
        previousState,
        settings,
      );

      if (isSameLiveCaptionRouteState(nextRouteState, previousRouteState)) {
        return;
      }

      routeState = nextRouteState;
      scheduleSync();
    });

    const settingsListenerIds = addFloatingOverlaySettingsListeners(
      store,
      () => {
        const nextSettings = getFloatingOverlaySettingsFromStore(store);
        const nextRouteState = getLiveCaptionRouteState(
          listenerStore.getState(),
          nextSettings,
        );

        settings = nextSettings;
        if (isSameLiveCaptionRouteState(nextRouteState, routeState)) {
          return;
        }

        routeState = nextRouteState;
        scheduleSync();
      },
    );

    return () => {
      cancelled = true;
      unsubscribe();
      removeSettingsListeners(store, settingsListenerIds);
      void hideLiveCaptionPanel();
    };
  });

  return null;
}

function FloatingMeetingWindowSync({
  store,
}: {
  store: SettingsStore | undefined;
}) {
  useMountEffect(() => {
    let settings = getFloatingOverlaySettingsFromStore(store);
    let routeState = getCurrentFloatingRouteState(
      listenerStore.getState(),
      undefined,
      settings,
    );
    let syncQueued = false;
    let cancelled = false;
    let shownSessionId: string | null = null;
    let nativeCommandsUnavailable = false;
    const unlisteners: Array<() => void> = [];

    const shouldContinue = () => !cancelled;

    const sync = async () => {
      if (!shouldContinue()) {
        return;
      }

      if (nativeCommandsUnavailable && routeState) {
        return;
      }

      const nextShownSessionId = await syncFloatingMeetingWindow(
        routeState,
        shownSessionId,
        shouldContinue,
      );
      if (!shouldContinue()) {
        await hideFloatingMeetingPanel();
        return;
      }

      if (nextShownSessionId === "unavailable") {
        nativeCommandsUnavailable = true;
        return;
      }

      shownSessionId = nextShownSessionId;
    };

    const scheduleSync = () => {
      if (syncQueued) {
        return;
      }

      syncQueued = true;
      queueMicrotask(() => {
        syncQueued = false;
        if (cancelled) {
          return;
        }

        void sync();
      });
    };

    windowsEvents.floatingBarStop
      .listen(() => {
        void hideFloatingMeetingPanel();
        listenerStore.getState().stop();
      })
      .then((unlisten) => {
        if (cancelled) {
          unlisten();
          return;
        }

        unlisteners.push(unlisten);
      });

    windowsEvents.floatingBarOpenMain
      .listen(() => {
        void windowsCommands.windowShow({ type: "main" });
      })
      .then((unlisten) => {
        if (cancelled) {
          unlisten();
          return;
        }

        unlisteners.push(unlisten);
      });

    scheduleSync();

    const unsubscribe = listenerStore.subscribe((state, previousState) => {
      const colorScheme = getCurrentFloatingBarColorScheme();
      const nextRouteState = getFloatingRouteState(state, {
        colorScheme,
        settings,
      });
      const previousRouteState = getFloatingRouteState(previousState, {
        colorScheme,
        settings,
      });

      if (isSameFloatingRouteState(nextRouteState, previousRouteState)) {
        return;
      }

      routeState = nextRouteState;
      scheduleSync();
    });

    const settingsListenerIds = addFloatingOverlaySettingsListeners(
      store,
      () => {
        const nextSettings = getFloatingOverlaySettingsFromStore(store);
        const nextRouteState = getCurrentFloatingRouteState(
          listenerStore.getState(),
          undefined,
          nextSettings,
        );

        settings = nextSettings;
        if (isSameFloatingRouteState(nextRouteState, routeState)) {
          return;
        }

        routeState = nextRouteState;
        scheduleSync();
      },
    );

    const unsubscribeAppliedTheme = subscribeToAppliedTheme(() => {
      const nextRouteState = getCurrentFloatingRouteState(
        listenerStore.getState(),
        undefined,
        settings,
      );

      if (isSameFloatingRouteState(nextRouteState, routeState)) {
        return;
      }

      routeState = nextRouteState;
      scheduleSync();
    });

    return () => {
      cancelled = true;
      unsubscribe();
      unsubscribeAppliedTheme();
      removeSettingsListeners(store, settingsListenerIds);
      unlisteners.forEach((unlisten) => unlisten());
      void hideFloatingMeetingPanel();
    };
  });

  return null;
}

export function getFloatingRouteState(
  state: ListenerState,
  {
    sessionId,
    colorScheme = "dark",
    settings = DEFAULT_FLOATING_OVERLAY_SETTINGS,
  }: {
    sessionId?: string;
    colorScheme?: FloatingBarColorScheme;
    settings?: FloatingOverlaySettings;
  } = {},
): FloatingRouteState | null {
  if (state.live.status !== "active") {
    return null;
  }

  if (!state.live.sessionId) {
    return null;
  }

  if (sessionId && state.live.sessionId !== sessionId) {
    return null;
  }

  return {
    sessionId: state.live.sessionId,
    amplitude: Math.min(
      Math.hypot(state.live.amplitude.mic, state.live.amplitude.speaker),
      1,
    ),
    status: state.live.degraded || state.live.lastError ? "error" : "recording",
    colorScheme,
    opacity: settings.floatingBarOpacity,
    liveCaptionOpacity: settings.liveCaptionOpacity,
    liveCaptionPosition: settings.liveCaptionPosition,
    liveCaptionMinimized: settings.liveCaptionMinimized,
  };
}

function getCurrentFloatingRouteState(
  state: ListenerState,
  sessionId?: string,
  settings: FloatingOverlaySettings = DEFAULT_FLOATING_OVERLAY_SETTINGS,
): FloatingRouteState | null {
  return getFloatingRouteState(state, {
    sessionId,
    colorScheme: getCurrentFloatingBarColorScheme(),
    settings,
  });
}

export function getLiveCaptionRouteState(
  state: ListenerState,
  settings: FloatingOverlaySettings = DEFAULT_FLOATING_OVERLAY_SETTINGS,
): LiveCaptionRouteState | null {
  if (state.live.status !== "active") {
    return null;
  }

  if (!state.live.sessionId) {
    return null;
  }

  if (state.live.liveTranscriptionActive !== true) {
    return null;
  }

  const text = state.liveCaptionText.trim();
  if (!text && !settings.liveCaptionMinimized) {
    return null;
  }

  return {
    sessionId: state.live.sessionId,
    text,
    opacity: settings.liveCaptionOpacity,
    position: settings.liveCaptionPosition,
    minimized: settings.liveCaptionMinimized,
  };
}

function getCurrentLiveCaptionRouteState(
  state: ListenerState,
  settings: FloatingOverlaySettings = DEFAULT_FLOATING_OVERLAY_SETTINGS,
): LiveCaptionRouteState | null {
  return getLiveCaptionRouteState(state, settings);
}

function subscribeToAppliedTheme(onStoreChange: () => void) {
  if (
    typeof document === "undefined" ||
    typeof MutationObserver === "undefined"
  ) {
    return () => {};
  }

  const observer = new MutationObserver(onStoreChange);
  observer.observe(document.documentElement, {
    attributeFilter: ["class"],
    attributes: true,
  });
  return () => observer.disconnect();
}

function addFloatingOverlaySettingsListeners(
  store: SettingsStore | undefined,
  onChange: () => void,
) {
  if (!store) {
    return [];
  }

  return FLOATING_OVERLAY_SETTING_KEYS.map((key) =>
    store.addValueListener(key, onChange),
  );
}

function removeSettingsListeners(
  store: SettingsStore | undefined,
  listenerIds: string[],
) {
  if (!store) {
    return;
  }

  for (const listenerId of listenerIds) {
    store.delListener(listenerId);
  }
}

export function getCurrentFloatingBarColorScheme(): FloatingBarColorScheme {
  if (typeof document === "undefined") {
    return "dark";
  }

  return document.documentElement.classList.contains("dark") ? "dark" : "light";
}

function isSameFloatingRouteState(
  left: FloatingRouteState | null,
  right: FloatingRouteState | null,
) {
  return (
    left?.sessionId === right?.sessionId &&
    left?.amplitude === right?.amplitude &&
    left?.status === right?.status &&
    left?.colorScheme === right?.colorScheme &&
    left?.opacity === right?.opacity &&
    left?.liveCaptionOpacity === right?.liveCaptionOpacity &&
    left?.liveCaptionPosition === right?.liveCaptionPosition &&
    left?.liveCaptionMinimized === right?.liveCaptionMinimized
  );
}

function isSameLiveCaptionRouteState(
  left: LiveCaptionRouteState | null,
  right: LiveCaptionRouteState | null,
) {
  return (
    left?.sessionId === right?.sessionId &&
    left?.text === right?.text &&
    left?.opacity === right?.opacity &&
    left?.position === right?.position &&
    left?.minimized === right?.minimized
  );
}

async function syncFloatingMeetingWindow(
  routeState: FloatingRouteState | null,
  shownSessionId: string | null,
  shouldContinue: () => boolean,
): Promise<string | null | "unavailable"> {
  if (!shouldContinue()) {
    return null;
  }

  if (!routeState) {
    await hideFloatingMeetingPanel();
    return null;
  }

  const ready = await showFloatingMeetingWindow(
    routeState,
    shownSessionId !== routeState.sessionId,
    shouldContinue,
  );
  if (!shouldContinue()) {
    await hideFloatingMeetingPanel();
    return null;
  }

  return ready ? routeState.sessionId : "unavailable";
}

async function syncLiveCaptionWindow(
  routeState: LiveCaptionRouteState | null,
  shownSessionId: string | null,
  shouldContinue: () => boolean,
): Promise<string | null | "unavailable"> {
  if (!shouldContinue()) {
    return null;
  }

  if (!routeState) {
    await hideLiveCaptionPanel();
    return null;
  }

  const ready = await showLiveCaptionWindow(
    routeState,
    shownSessionId !== routeState.sessionId,
    shouldContinue,
  );
  if (!shouldContinue()) {
    await hideLiveCaptionPanel();
    return null;
  }

  return ready ? routeState.sessionId : "unavailable";
}

async function showFloatingMeetingWindow(
  routeState: FloatingRouteState,
  shouldShow: boolean,
  shouldContinue: () => boolean = () => true,
): Promise<boolean> {
  if (!shouldContinue()) {
    return false;
  }

  if (shouldShow) {
    const showResult = await windowsCommands.floatingBarShow();
    if (!shouldContinue()) {
      await hideFloatingMeetingPanel();
      return false;
    }

    if (showResult.status === "error") {
      console.error("Failed to show floating meeting panel:", showResult.error);
      return false;
    }
  }

  const updateResult = await windowsCommands.floatingBarUpdate({
    amplitude: routeState.amplitude,
    status: routeState.status,
    colorScheme: routeState.colorScheme,
    opacity: routeState.opacity,
    liveCaptionOpacity: routeState.liveCaptionOpacity,
    liveCaptionPosition: routeState.liveCaptionPosition,
    liveCaptionMinimized: routeState.liveCaptionMinimized,
  });
  if (!shouldContinue()) {
    await hideFloatingMeetingPanel();
    return false;
  }

  if (updateResult.status === "error") {
    console.error(
      "Failed to update floating meeting panel:",
      updateResult.error,
    );
    return false;
  }

  return true;
}

async function showLiveCaptionWindow(
  routeState: LiveCaptionRouteState,
  shouldShow: boolean,
  shouldContinue: () => boolean = () => true,
): Promise<boolean> {
  if (!shouldContinue()) {
    return false;
  }

  if (shouldShow) {
    const showResult = await windowsCommands.liveCaptionShow();
    if (!shouldContinue()) {
      await hideLiveCaptionPanel();
      return false;
    }

    if (showResult.status === "error") {
      console.error("Failed to show live caption panel:", showResult.error);
      return false;
    }
  }

  const updateResult = await windowsCommands.liveCaptionUpdate({
    text: routeState.text,
    opacity: routeState.opacity,
    position: routeState.position,
    minimized: routeState.minimized,
  });
  if (!shouldContinue()) {
    await hideLiveCaptionPanel();
    return false;
  }

  if (updateResult.status === "error") {
    console.error("Failed to update live caption panel:", updateResult.error);
    return false;
  }

  return true;
}

function normalizeOpacity(value: unknown, fallback: number): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return fallback;
  }

  return Math.min(Math.max(value, 0.35), 0.95);
}

function normalizeLiveCaptionPosition(value: unknown): LiveCaptionPosition {
  if (typeof value === "string" && LIVE_CAPTION_POSITIONS.has(value)) {
    return value as LiveCaptionPosition;
  }

  return DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionPosition;
}

function getSettingsValuesFromNativeChange(change: FloatingBarSettingsChange) {
  const values: Partial<FloatingOverlaySettingsStorage> = {};

  if (
    change.floatingBarOpacity !== null &&
    change.floatingBarOpacity !== undefined
  ) {
    values.floating_bar_opacity = normalizeOpacity(
      change.floatingBarOpacity,
      DEFAULT_FLOATING_OVERLAY_SETTINGS.floatingBarOpacity,
    );
  }

  if (
    change.liveCaptionOpacity !== null &&
    change.liveCaptionOpacity !== undefined
  ) {
    values.live_caption_opacity = normalizeOpacity(
      change.liveCaptionOpacity,
      DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionOpacity,
    );
  }

  if (
    change.liveCaptionPosition !== null &&
    change.liveCaptionPosition !== undefined
  ) {
    values.live_caption_position = normalizeLiveCaptionPosition(
      change.liveCaptionPosition,
    );
  }

  if (
    change.liveCaptionMinimized !== null &&
    change.liveCaptionMinimized !== undefined
  ) {
    values.live_caption_minimized = change.liveCaptionMinimized === true;
  }

  return values;
}

export async function openFloatingMeetingPanel({
  sessionId,
  enabled,
}: {
  sessionId?: string;
  enabled: boolean;
}) {
  if (!enabled) {
    await hideFloatingMeetingPanel();
    return;
  }

  const routeState = getCurrentFloatingRouteState(
    listenerStore.getState(),
    sessionId,
  );

  if (!routeState) {
    return;
  }

  await showFloatingMeetingWindow(routeState, true);
}

export async function hideFloatingMeetingPanel() {
  const result = await windowsCommands.floatingBarHide();
  if (result.status === "error") {
    console.error("Failed to hide floating meeting panel:", result.error);
  }
}

export async function hideLiveCaptionPanel() {
  const result = await windowsCommands.liveCaptionHide();
  if (result.status === "error") {
    console.error("Failed to hide live caption panel:", result.error);
  }
}
