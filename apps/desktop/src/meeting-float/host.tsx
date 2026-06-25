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
export type SettingsStore = NonNullable<
  ReturnType<typeof settingsStore.UI.useStore>
>;
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
  liveCaptionWidth: number;
  liveCaptionLineCount: number;
  liveCaptionPosition: LiveCaptionPosition;
  liveCaptionMinimized: boolean;
  liveCaptionEnabled: boolean;
};
type FloatingOverlaySettingsStorage = Pick<
  GeneralStorage,
  | "floating_bar_opacity"
  | "live_caption_opacity"
  | "live_caption_width"
  | "live_caption_line_count"
  | "live_caption_position"
  | "live_caption_minimized"
  | "live_caption_enabled"
>;
type FloatingRouteState = {
  sessionId: string;
  amplitude: number;
  status: FloatingBarStatus;
  colorScheme: FloatingBarColorScheme;
  opacity: number;
  liveCaptionOpacity: number;
  liveCaptionWidth: number;
  liveCaptionLineCount: number;
  liveCaptionPosition: LiveCaptionPosition;
  liveCaptionMinimized: boolean;
  liveCaptionToggleVisible: boolean;
};
type LiveCaptionRouteState = {
  sessionId: string;
  text: string;
  opacity: number;
  width: number;
  lineCount: number;
  position: LiveCaptionPosition;
  minimized: boolean;
};

const DEFAULT_FLOATING_OVERLAY_SETTINGS: FloatingOverlaySettings = {
  floatingBarOpacity: 0.78,
  liveCaptionOpacity: 0.3,
  liveCaptionWidth: 440,
  liveCaptionLineCount: 1,
  liveCaptionPosition: "topCenter",
  liveCaptionMinimized: false,
  liveCaptionEnabled: true,
};

const FLOATING_BAR_MIN_OPACITY = 0.35;
const FLOATING_BAR_MAX_OPACITY = 0.95;
const LIVE_CAPTION_MIN_OPACITY = 0.05;
const LIVE_CAPTION_MAX_OPACITY = 1;
const LIVE_CAPTION_MIN_WIDTH = 260;
const LIVE_CAPTION_MAX_WIDTH = 640;
const LIVE_CAPTION_MIN_LINE_COUNT = 1;
const LIVE_CAPTION_MAX_LINE_COUNT = 4;
const LIVE_CAPTION_HORIZONTAL_PADDING_PX = 32;
const LIVE_CAPTION_AVERAGE_CHARACTER_WIDTH_PX = 7.8;

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
  "live_caption_width",
  "live_caption_line_count",
  "live_caption_position",
  "live_caption_minimized",
  "live_caption_enabled",
  "current_stt_provider",
  "current_stt_model",
] as const;

export function FloatingMeetingWindowHost() {
  const floatingBarEnabled = useConfigValue("floating_bar_enabled");
  const store = settingsStore.UI.useStore(settingsStore.STORE_ID);

  return (
    <>
      <FloatingOverlaySettingsEventSync />
      <LiveCaptionDefaultVisibilitySync store={store} />
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
      FLOATING_BAR_MIN_OPACITY,
      FLOATING_BAR_MAX_OPACITY,
    ),
    liveCaptionOpacity: normalizeOpacity(
      store?.getValue("live_caption_opacity"),
      DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionOpacity,
      LIVE_CAPTION_MIN_OPACITY,
      LIVE_CAPTION_MAX_OPACITY,
    ),
    liveCaptionWidth: normalizeNumber(
      store?.getValue("live_caption_width"),
      DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionWidth,
      LIVE_CAPTION_MIN_WIDTH,
      LIVE_CAPTION_MAX_WIDTH,
    ),
    liveCaptionLineCount: normalizeInteger(
      store?.getValue("live_caption_line_count"),
      DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionLineCount,
      LIVE_CAPTION_MIN_LINE_COUNT,
      LIVE_CAPTION_MAX_LINE_COUNT,
    ),
    liveCaptionPosition: normalizeLiveCaptionPosition(
      store?.getValue("live_caption_position"),
    ),
    liveCaptionMinimized: store?.getValue("live_caption_minimized") === true,
    liveCaptionEnabled: store?.getValue("live_caption_enabled") !== false,
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

function LiveCaptionDefaultVisibilitySync({
  store,
}: {
  store: SettingsStore | undefined;
}) {
  useMountEffect(() => {
    let appliedSessionId: string | null = null;

    const applyDefaultVisibility = (state: ListenerState) => {
      if (!store || state.live.status !== "active" || !state.live.sessionId) {
        appliedSessionId = null;
        return;
      }

      if (state.live.liveTranscriptionActive !== true) {
        return;
      }

      if (appliedSessionId === state.live.sessionId) {
        return;
      }

      appliedSessionId = state.live.sessionId;
      store.setValue(
        "live_caption_minimized",
        getLiveCaptionMinimizedForSessionDefault({
          liveCaptionEnabled: store.getValue("live_caption_enabled") !== false,
        }),
      );
    };

    applyDefaultVisibility(listenerStore.getState());

    const unsubscribe = listenerStore.subscribe((state) => {
      applyDefaultVisibility(state);
    });

    return () => {
      unsubscribe();
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
    const unlisteners: Array<() => void> = [];

    const shouldContinue = () => !cancelled;
    const updateSettings = (nextSettings: FloatingOverlaySettings) => {
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
    };

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
        updateSettings(getFloatingOverlaySettingsFromStore(store));
      },
    );
    windowsEvents.floatingBarSettingsChange
      .listen((event) => {
        if (cancelled) {
          return;
        }

        updateSettings(
          mergeFloatingOverlaySettings(
            settings,
            getSettingsValuesFromNativeChange(event.payload),
          ),
        );
      })
      .then((unlisten) => {
        if (cancelled) {
          unlisten();
          return;
        }

        unlisteners.push(unlisten);
      });

    return () => {
      cancelled = true;
      unsubscribe();
      removeSettingsListeners(store, settingsListenerIds);
      unlisteners.forEach((unlisten) => unlisten());
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
      getFloatingLiveCaptionToggleVisible(listenerStore.getState(), store),
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
        liveCaptionToggleVisible: getFloatingLiveCaptionToggleVisible(
          state,
          store,
        ),
      });
      const previousRouteState = getFloatingRouteState(previousState, {
        colorScheme,
        settings,
        liveCaptionToggleVisible: getFloatingLiveCaptionToggleVisible(
          previousState,
          store,
        ),
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
          getFloatingLiveCaptionToggleVisible(listenerStore.getState(), store),
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
        getFloatingLiveCaptionToggleVisible(listenerStore.getState(), store),
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
    liveCaptionToggleVisible = false,
  }: {
    sessionId?: string;
    colorScheme?: FloatingBarColorScheme;
    settings?: FloatingOverlaySettings;
    liveCaptionToggleVisible?: boolean;
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
    liveCaptionWidth: settings.liveCaptionWidth,
    liveCaptionLineCount: settings.liveCaptionLineCount,
    liveCaptionPosition: settings.liveCaptionPosition,
    liveCaptionMinimized: settings.liveCaptionMinimized,
    liveCaptionToggleVisible,
  };
}

function getCurrentFloatingRouteState(
  state: ListenerState,
  sessionId?: string,
  settings: FloatingOverlaySettings = DEFAULT_FLOATING_OVERLAY_SETTINGS,
  liveCaptionToggleVisible = false,
): FloatingRouteState | null {
  return getFloatingRouteState(state, {
    sessionId,
    colorScheme: getCurrentFloatingBarColorScheme(),
    settings,
    liveCaptionToggleVisible,
  });
}

export function shouldShowFloatingLiveCaptionToggle({
  liveTranscriptionActive,
}: {
  provider?: string | null;
  model?: string | null;
  liveTranscriptionActive: boolean;
}) {
  return liveTranscriptionActive;
}

export function getLiveCaptionMinimizedForSessionDefault({
  liveCaptionEnabled,
}: {
  liveCaptionEnabled: boolean;
}) {
  return !liveCaptionEnabled;
}

function getFloatingLiveCaptionToggleVisible(
  state: ListenerState,
  store: SettingsStore | undefined,
) {
  const provider = store?.getValue("current_stt_provider");
  const model = store?.getValue("current_stt_model");

  return shouldShowFloatingLiveCaptionToggle({
    provider: typeof provider === "string" ? provider : undefined,
    model: typeof model === "string" ? model : undefined,
    liveTranscriptionActive: state.live.liveTranscriptionActive === true,
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

  if (settings.liveCaptionMinimized) {
    return null;
  }

  const text = getLiveCaptionDisplayText(state.liveCaptionText, settings);

  return {
    sessionId: state.live.sessionId,
    text,
    opacity: settings.liveCaptionOpacity,
    width: settings.liveCaptionWidth,
    lineCount: settings.liveCaptionLineCount,
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
    left?.liveCaptionWidth === right?.liveCaptionWidth &&
    left?.liveCaptionLineCount === right?.liveCaptionLineCount &&
    left?.liveCaptionPosition === right?.liveCaptionPosition &&
    left?.liveCaptionMinimized === right?.liveCaptionMinimized &&
    left?.liveCaptionToggleVisible === right?.liveCaptionToggleVisible
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
    left?.width === right?.width &&
    left?.lineCount === right?.lineCount &&
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
    liveCaptionWidth: routeState.liveCaptionWidth,
    liveCaptionLineCount: routeState.liveCaptionLineCount,
    liveCaptionPosition: routeState.liveCaptionPosition,
    liveCaptionMinimized: routeState.liveCaptionMinimized,
    liveCaptionToggleVisible: routeState.liveCaptionToggleVisible,
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
    width: routeState.width,
    lineCount: routeState.lineCount,
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

export function getLiveCaptionDisplayText(
  text: string,
  settings: Pick<
    FloatingOverlaySettings,
    "liveCaptionWidth" | "liveCaptionLineCount"
  > = DEFAULT_FLOATING_OVERLAY_SETTINGS,
) {
  const normalizedText = text.trim().replace(/\s+/g, " ");
  if (!normalizedText) {
    return "";
  }

  const contentWidth = Math.max(
    settings.liveCaptionWidth - LIVE_CAPTION_HORIZONTAL_PADDING_PX,
    LIVE_CAPTION_MIN_WIDTH - LIVE_CAPTION_HORIZONTAL_PADDING_PX,
  );
  const charactersPerLine = Math.max(
    12,
    Math.floor(contentWidth / LIVE_CAPTION_AVERAGE_CHARACTER_WIDTH_PX),
  );
  const maxCharacters = Math.max(
    24,
    charactersPerLine * settings.liveCaptionLineCount,
  );

  if (normalizedText.length <= maxCharacters) {
    return normalizedText;
  }

  return `... ${getTextSuffixAtWordBoundary(normalizedText, maxCharacters - 4)}`;
}

function getTextSuffixAtWordBoundary(text: string, maxCharacters: number) {
  const suffix = text.slice(-maxCharacters).trimStart();
  const firstWhitespaceIndex = suffix.search(/\s/);
  if (firstWhitespaceIndex > 0 && firstWhitespaceIndex < suffix.length - 1) {
    return suffix.slice(firstWhitespaceIndex).trimStart();
  }

  return suffix;
}
function normalizeOpacity(
  value: unknown,
  fallback: number,
  min: number,
  max: number,
): number {
  return normalizeNumber(value, fallback, min, max);
}

function normalizeNumber(
  value: unknown,
  fallback: number,
  min: number,
  max: number,
): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return fallback;
  }

  return Math.min(Math.max(value, min), max);
}

function normalizeInteger(
  value: unknown,
  fallback: number,
  min: number,
  max: number,
): number {
  return Math.round(normalizeNumber(value, fallback, min, max));
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
      FLOATING_BAR_MIN_OPACITY,
      FLOATING_BAR_MAX_OPACITY,
    );
  }

  if (
    change.liveCaptionOpacity !== null &&
    change.liveCaptionOpacity !== undefined
  ) {
    values.live_caption_opacity = normalizeOpacity(
      change.liveCaptionOpacity,
      DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionOpacity,
      LIVE_CAPTION_MIN_OPACITY,
      LIVE_CAPTION_MAX_OPACITY,
    );
  }

  if (
    change.liveCaptionWidth !== null &&
    change.liveCaptionWidth !== undefined
  ) {
    values.live_caption_width = normalizeNumber(
      change.liveCaptionWidth,
      DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionWidth,
      LIVE_CAPTION_MIN_WIDTH,
      LIVE_CAPTION_MAX_WIDTH,
    );
  }

  if (
    change.liveCaptionLineCount !== null &&
    change.liveCaptionLineCount !== undefined
  ) {
    values.live_caption_line_count = normalizeInteger(
      change.liveCaptionLineCount,
      DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionLineCount,
      LIVE_CAPTION_MIN_LINE_COUNT,
      LIVE_CAPTION_MAX_LINE_COUNT,
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

function mergeFloatingOverlaySettings(
  settings: FloatingOverlaySettings,
  values: Partial<FloatingOverlaySettingsStorage>,
): FloatingOverlaySettings {
  return {
    ...settings,
    floatingBarOpacity:
      values.floating_bar_opacity ?? settings.floatingBarOpacity,
    liveCaptionOpacity:
      values.live_caption_opacity ?? settings.liveCaptionOpacity,
    liveCaptionWidth: values.live_caption_width ?? settings.liveCaptionWidth,
    liveCaptionLineCount:
      values.live_caption_line_count ?? settings.liveCaptionLineCount,
    liveCaptionPosition:
      values.live_caption_position === undefined
        ? settings.liveCaptionPosition
        : normalizeLiveCaptionPosition(values.live_caption_position),
    liveCaptionMinimized:
      values.live_caption_minimized ?? settings.liveCaptionMinimized,
  };
}

export async function openFloatingMeetingPanel({
  sessionId,
  enabled,
  store,
}: {
  sessionId?: string;
  enabled: boolean;
  store?: SettingsStore;
}) {
  if (!enabled) {
    await hideFloatingMeetingPanel();
    return;
  }

  const state = listenerStore.getState();
  const routeState = getCurrentFloatingRouteState(
    state,
    sessionId,
    getFloatingOverlaySettingsFromStore(store),
    getFloatingLiveCaptionToggleVisible(state, store),
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
