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
  createFloatingMeetingWindowSynchronizer,
  hideFloatingMeetingPanel,
  hideLiveCaptionPanel,
  showFloatingMeetingWindow,
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
    const initialListenerState = listenerStore.getState();
    let routeState = getCurrentFloatingRouteState(
      initialListenerState,
      undefined,
      settingsRef.current,
      getFloatingLiveCaptionToggleVisible(initialListenerState),
      meetingData,
    );
    let cancelled = false;
    const windowSynchronizer = createFloatingMeetingWindowSynchronizer();
    let unsubscribeMeetingData: (() => Promise<void>) | null = null;
    const unlisteners: Array<() => void> = [];

    const updateRouteState = (nextRouteState: FloatingRouteState | null) => {
      if (isSameFloatingRouteState(nextRouteState, routeState)) {
        return;
      }

      routeState = nextRouteState;
      windowSynchronizer.update(routeState);
    };
    const refreshCurrentRouteState = (refreshTranscriptBubbles = false) => {
      const state = listenerStore.getState();
      const transcriptBubbles =
        !refreshTranscriptBubbles &&
        routeState?.sessionId === state.live.sessionId
          ? routeState.transcriptBubbles
          : undefined;
      updateRouteState(
        getCurrentFloatingRouteState(
          state,
          undefined,
          settingsRef.current,
          getFloatingLiveCaptionToggleVisible(state),
          meetingData,
          transcriptBubbles,
        ),
      );
    };
    refreshSettingsRef.current = refreshCurrentRouteState;

    windowsEvents.floatingBarStop
      .listen(() => {
        windowSynchronizer.update(null);
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

    windowSynchronizer.update(routeState);

    const unsubscribe = listenerStore.subscribe((state, previousState) => {
      if (!haveFloatingRouteInputsChanged(state, previousState)) {
        return;
      }

      refreshCurrentRouteState(
        state.liveSegments !== previousState.liveSegments ||
          state.live.sessionId !== previousState.live.sessionId,
      );
    });

    void subscribeMeetingFloatData(
      (nextData) => {
        meetingData = nextData;
        refreshCurrentRouteState(true);
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
      void windowSynchronizer.dispose();
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
  transcriptBubbles?: FloatingRouteState["transcriptBubbles"],
): FloatingRouteState | null {
  return getFloatingRouteState(state, {
    sessionId,
    colorScheme: getCurrentFloatingBarColorScheme(),
    settings,
    liveCaptionToggleVisible,
    sessionTitle: getFloatingSessionTitle(state, meetingData),
    speakerLabelContext: getFloatingSpeakerLabelContext(state, meetingData),
    transcriptBubbles,
  });
}

function haveFloatingRouteInputsChanged(
  state: ListenerState,
  previousState: ListenerState,
) {
  return (
    state.live.status !== previousState.live.status ||
    state.live.sessionId !== previousState.live.sessionId ||
    state.live.amplitude.mic !== previousState.live.amplitude.mic ||
    state.live.amplitude.speaker !== previousState.live.amplitude.speaker ||
    Boolean(state.live.degraded) !== Boolean(previousState.live.degraded) ||
    Boolean(state.live.lastError) !== Boolean(previousState.live.lastError) ||
    state.live.liveTranscriptionActive !==
      previousState.live.liveTranscriptionActive ||
    state.liveSegments !== previousState.liveSegments
  );
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
