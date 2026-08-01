import { platform } from "@tauri-apps/plugin-os";
import { useRef } from "react";

import {
  commands as windowsCommands,
  events as windowsEvents,
} from "@anlg/plugin-windows";

import {
  createMeetingFloatLabelContext,
  loadMeetingFloatData,
  type MeetingFloatData,
  subscribeMeetingFloatData,
} from "./hooks";
import {
  getCurrentFloatingBarColorScheme,
  getFloatingLiveCaptionToggleVisible,
  getFloatingRouteState,
  isSameFloatingRouteState,
  type FloatingRouteState,
  type ListenerState,
} from "./route-state";
import {
  DEFAULT_FLOATING_OVERLAY_SETTINGS,
  FLOATING_OVERLAY_SETTING_KEYS,
  getFloatingOverlaySettings,
  getSettingsValuesFromNativeChange,
  type FloatingOverlaySettings,
} from "./settings";
import {
  hideFloatingMeetingPanel,
  hideLiveCaptionPanel,
  showFloatingMeetingWindow,
  syncFloatingMeetingWindow,
} from "./window-panel";

import {
  getStoredSettingValues,
  setSettingValue,
  useSetSettingValues,
} from "~/settings/queries";
import { useConfigValue, useConfigValues } from "~/shared/config";
import { useLatestRef } from "~/shared/hooks/useLatestRef";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { listenerStore } from "~/store/zustand/listener/instance";
import type { RenderLabelContext } from "~/stt/live-segment";

export {
  getCurrentFloatingBarColorScheme,
  getFloatingRouteState,
  getFloatingTranscriptBubbles,
  getLiveCaptionDisplayText,
  getLiveCaptionRouteState,
  shouldShowFloatingLiveCaptionToggle,
} from "./route-state";
export { hideFloatingMeetingPanel, hideLiveCaptionPanel } from "./window-panel";

export function FloatingMeetingWindowHost() {
  const floatingBarEnabled = useConfigValue("floating_bar_enabled");
  const storedSettings = useConfigValues(FLOATING_OVERLAY_SETTING_KEYS);
  const overlaySettings = getFloatingOverlaySettings(storedSettings);
  const floatingOverlaySupported = platform() === "macos";

  return (
    <>
      {floatingOverlaySupported && (
        <>
          <FloatingOverlaySettingsEventSync />
          <LiveCaptionDefaultVisibilitySync />
        </>
      )}
      {floatingOverlaySupported && floatingBarEnabled ? (
        <FloatingMeetingWindowSync settings={overlaySettings} />
      ) : (
        <FloatingMeetingWindowDisabled />
      )}
      <LiveCaptionWindowDisabled />
    </>
  );
}

function FloatingOverlaySettingsEventSync() {
  const setSettingValues = useSetSettingValues();

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

        setSettingValues(values);
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

function LiveCaptionDefaultVisibilitySync() {
  useMountEffect(() => {
    let appliedSessionId: string | null = null;

    const applyDefaultVisibility = (state: ListenerState) => {
      if (state.live.status !== "active" || !state.live.sessionId) {
        appliedSessionId = null;
        return;
      }

      if (appliedSessionId === state.live.sessionId) {
        return;
      }

      appliedSessionId = state.live.sessionId;
      void setSettingValue("live_caption_minimized", true);
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

function LiveCaptionWindowDisabled() {
  useMountEffect(() => {
    void hideLiveCaptionPanel();
  });

  return null;
}

function FloatingMeetingWindowSync({
  settings,
}: {
  settings: FloatingOverlaySettings;
}) {
  const settingsRef = useLatestRef(settings);
  const refreshSettingsRef = useRef<() => void>(() => {});

  useMountEffect(() => {
    let meetingData: MeetingFloatData = { sessions: {}, humanNames: {} };
    let routeState = getCurrentFloatingRouteState(
      listenerStore.getState(),
      undefined,
      settingsRef.current,
      getFloatingLiveCaptionToggleVisible(listenerStore.getState()),
      meetingData,
    );
    let syncQueued = false;
    let cancelled = false;
    let shownSessionId: string | null = null;
    let nativeCommandsUnavailable = false;
    let unsubscribeMeetingData: (() => Promise<void>) | null = null;
    const unlisteners: Array<() => void> = [];

    const shouldContinue = () => !cancelled;
    const updateRouteState = (nextRouteState: FloatingRouteState | null) => {
      if (isSameFloatingRouteState(nextRouteState, routeState)) {
        return;
      }

      routeState = nextRouteState;
      scheduleSync();
    };
    const refreshCurrentRouteState = () => {
      updateRouteState(
        getCurrentFloatingRouteState(
          listenerStore.getState(),
          undefined,
          settingsRef.current,
          getFloatingLiveCaptionToggleVisible(listenerStore.getState()),
          meetingData,
        ),
      );
    };
    refreshSettingsRef.current = refreshCurrentRouteState;

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
      .listen(async () => {
        await windowsCommands.windowShow({ type: "main" });
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
        settings: settingsRef.current,
        liveCaptionToggleVisible: getFloatingLiveCaptionToggleVisible(state),
        sessionTitle: getFloatingSessionTitle(state, meetingData),
        speakerLabelContext: getFloatingSpeakerLabelContext(state, meetingData),
      });
      const previousRouteState = getFloatingRouteState(previousState, {
        colorScheme,
        settings: settingsRef.current,
        liveCaptionToggleVisible:
          getFloatingLiveCaptionToggleVisible(previousState),
        sessionTitle: getFloatingSessionTitle(previousState, meetingData),
        speakerLabelContext: getFloatingSpeakerLabelContext(
          previousState,
          meetingData,
        ),
      });

      if (!isSameFloatingRouteState(nextRouteState, previousRouteState)) {
        updateRouteState(nextRouteState);
      }
    });

    void subscribeMeetingFloatData(
      (nextData) => {
        meetingData = nextData;
        refreshCurrentRouteState();
      },
      (error) => {
        console.error("Failed to read floating meeting data:", error);
      },
    )
      .then((unsubscribe) => {
        if (cancelled) {
          void unsubscribe();
        } else {
          unsubscribeMeetingData = unsubscribe;
        }
      })
      .catch((error) => {
        console.error("Failed to subscribe to floating meeting data:", error);
      });

    const unsubscribeAppliedTheme = subscribeToAppliedTheme(() => {
      refreshCurrentRouteState();
    });

    return () => {
      cancelled = true;
      refreshSettingsRef.current = () => {};
      unsubscribe();
      unsubscribeAppliedTheme();
      void unsubscribeMeetingData?.();
      unlisteners.forEach((unlisten) => unlisten());
      void hideFloatingMeetingPanel();
    };
  });

  return (
    <FloatingMeetingWindowSettingsSync
      key={JSON.stringify(settings)}
      onSettingsChange={() => refreshSettingsRef.current()}
    />
  );
}

function FloatingMeetingWindowSettingsSync({
  onSettingsChange,
}: {
  onSettingsChange: () => void;
}) {
  useMountEffect(onSettingsChange);
  return null;
}

function getCurrentFloatingRouteState(
  state: ListenerState,
  sessionId?: string,
  settings: FloatingOverlaySettings = DEFAULT_FLOATING_OVERLAY_SETTINGS,
  liveCaptionToggleVisible = false,
  meetingData?: MeetingFloatData,
): FloatingRouteState | null {
  return getFloatingRouteState(state, {
    sessionId,
    colorScheme: getCurrentFloatingBarColorScheme(),
    settings,
    liveCaptionToggleVisible,
    sessionTitle: getFloatingSessionTitle(state, meetingData),
    speakerLabelContext: getFloatingSpeakerLabelContext(state, meetingData),
  });
}

function getFloatingSessionTitle(
  state: ListenerState,
  meetingData: MeetingFloatData | undefined,
) {
  const sessionId = state.live.sessionId;
  if (!sessionId) {
    return null;
  }

  return meetingData?.sessions[sessionId]?.title ?? null;
}

function getFloatingSpeakerLabelContext(
  state: ListenerState,
  meetingData: MeetingFloatData | undefined,
): RenderLabelContext | undefined {
  if (!meetingData || !state.live.sessionId) {
    return undefined;
  }

  return createMeetingFloatLabelContext(meetingData, state.live.sessionId);
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

  const state = listenerStore.getState();
  const [{ values }, meetingData] = await Promise.all([
    getStoredSettingValues(),
    loadMeetingFloatData(),
  ]);
  const routeState = getCurrentFloatingRouteState(
    state,
    sessionId,
    getFloatingOverlaySettings(values),
    getFloatingLiveCaptionToggleVisible(state),
    meetingData,
  );

  if (!routeState) {
    return;
  }

  await showFloatingMeetingWindow(routeState, true);
}
