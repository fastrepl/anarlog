import { useRef } from "react";

import {
  commands as windowsCommands,
  events as windowsEvents,
} from "@anlg/plugin-windows";

import {
  createMeetingFloatLabelContext,
  createMeetingFloatRenderRequest,
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
  createFloatingSpeakerLabeler,
  type FloatingSpeakerLabels,
} from "./speaker-labels";
import { isFloatingBarSupported } from "./support";
import {
  createFloatingMeetingWindowSynchronizer,
  hideFloatingMeetingPanel,
  hideLiveCaptionPanel,
  showFloatingMeetingWindow,
} from "./window-panel";

import { getDictationPanelState } from "~/dictation/panel";
import { useDictationStatus } from "~/dictation/state";
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
  shouldShowFloatingLiveCaptionToggle,
} from "./route-state";

export function FloatingMeetingWindowHost() {
  const floatingBarEnabled = useConfigValue("floating_bar_enabled");
  const storedSettings = useConfigValues(FLOATING_OVERLAY_SETTING_KEYS);
  const overlaySettings = getFloatingOverlaySettings(storedSettings);
  const floatingOverlaySupported = isFloatingBarSupported();

  return (
    <>
      {floatingOverlaySupported && (
        <>
          <FloatingOverlaySettingsEventSync />
          <LiveCaptionDefaultVisibilitySync />
        </>
      )}
      {floatingOverlaySupported ? (
        <FloatingMeetingWindowSync
          settings={overlaySettings}
          enabled={floatingBarEnabled}
        />
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
  enabled,
}: {
  settings: FloatingOverlaySettings;
  enabled: boolean;
}) {
  const settingsRef = useLatestRef(settings);
  const enabledRef = useLatestRef(enabled);
  const refreshSettingsRef = useRef<() => void>(() => {});

  useMountEffect(() => {
    let meetingData: MeetingFloatData = { sessions: {}, humanNames: {} };
    let routeState: FloatingRouteState | null = null;
    let hasRouteState = false;
    let cancelled = false;
    const windowSynchronizer = createFloatingMeetingWindowSynchronizer(
      (state) => {
        useDictationStatus.setState({
          presentedOwner: state?.dictation?.sessionId ?? null,
        });
      },
    );
    let unsubscribeMeetingData: (() => Promise<void>) | null = null;
    const unlisteners: Array<() => void> = [];
    const speakerLabeler = createFloatingSpeakerLabeler(() => {
      if (!cancelled) refreshCurrentRouteState(true);
    });

    const updateRouteState = (nextRouteState: FloatingRouteState | null) => {
      if (
        hasRouteState &&
        isSameFloatingRouteState(nextRouteState, routeState)
      ) {
        return;
      }

      hasRouteState = true;
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
        state.live.status === "inactive" && !state.live.loading
          ? getDictationPanelState()
          : enabledRef.current
            ? getCurrentFloatingRouteState(
                state,
                undefined,
                settingsRef.current,
                getFloatingLiveCaptionToggleVisible(state),
                meetingData,
                transcriptBubbles,
                speakerLabeler.labels,
              )
            : null,
      );
    };
    const resolveSpeakerLabels = () => {
      const state = listenerStore.getState();
      const sessionId =
        enabledRef.current && state.live.status === "active"
          ? state.live.sessionId
          : null;
      speakerLabeler.update(
        sessionId,
        state.liveSegments,
        sessionId
          ? createMeetingFloatRenderRequest(meetingData, sessionId)
          : null,
      );
    };
    refreshSettingsRef.current = refreshCurrentRouteState;

    windowsEvents.floatingBarStop
      .listen(() => {
        if (
          !enabledRef.current ||
          routeState?.dictation ||
          listenerStore.getState().live.status !== "active"
        )
          return;
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

    windowsEvents.floatingBarDictationAction
      .listen(({ payload }) => {
        if (cancelled) return;
        const state = useDictationStatus.getState();
        if (state.phase === "idle" || state.owner !== payload.sessionId) return;
        if (payload.action === "cancel") state.cancel?.();
        else if (payload.action === "finish") state.finish?.();
        else useDictationStatus.setState({ expanded: !state.expanded });
      })
      .then((unlisten) => {
        if (cancelled) unlisten();
        else unlisteners.push(unlisten);
      });
    resolveSpeakerLabels();
    refreshCurrentRouteState();
    const unsubscribeDictation = useDictationStatus.subscribe(() =>
      refreshCurrentRouteState(),
    );

    const unsubscribe = listenerStore.subscribe((state, previousState) => {
      if (!haveFloatingRouteInputsChanged(state, previousState)) {
        return;
      }

      const transcriptChanged =
        state.liveSegments !== previousState.liveSegments ||
        state.live.sessionId !== previousState.live.sessionId;
      if (transcriptChanged) {
        resolveSpeakerLabels();
      }
      refreshCurrentRouteState(transcriptChanged);
    });

    void subscribeMeetingFloatData(
      (nextData) => {
        meetingData = nextData;
        resolveSpeakerLabels();
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
      speakerLabeler.dispose();
      unsubscribe();
      unsubscribeDictation();
      unsubscribeAppliedTheme();
      void unsubscribeMeetingData?.();
      unlisteners.forEach((unlisten) => unlisten());
      void windowSynchronizer.dispose();
    };
  });

  return (
    <FloatingMeetingWindowSettingsSync
      key={JSON.stringify([settings, enabled])}
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
  speakerLabels?: FloatingSpeakerLabels,
): FloatingRouteState | null {
  return getFloatingRouteState(state, {
    sessionId,
    colorScheme: getCurrentFloatingBarColorScheme(),
    settings,
    liveCaptionToggleVisible,
    sessionTitle: getFloatingSessionTitle(state, meetingData),
    speakerLabelContext: getFloatingSpeakerLabelContext(state, meetingData),
    speakerLabels,
    transcriptBubbles,
  });
}

export function haveFloatingRouteInputsChanged(
  state: ListenerState,
  previousState: ListenerState,
) {
  return (
    state.live.status !== previousState.live.status ||
    state.live.sessionId !== previousState.live.sessionId ||
    state.live.loadingPhase !== previousState.live.loadingPhase ||
    state.live.lastErrorIsAudioRelated !==
      previousState.live.lastErrorIsAudioRelated ||
    state.live.amplitude.mic !== previousState.live.amplitude.mic ||
    state.live.amplitude.speaker !== previousState.live.amplitude.speaker ||
    state.live.degraded?.type !== previousState.live.degraded?.type ||
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
