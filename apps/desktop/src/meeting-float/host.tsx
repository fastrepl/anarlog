import { useRef } from "react";

import { commands as transcriptCommands } from "@anlg/plugin-transcription";
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
  type FloatingSpeakerLabels,
  type ListenerState,
} from "./route-state";
import {
  DEFAULT_FLOATING_OVERLAY_SETTINGS,
  FLOATING_OVERLAY_SETTING_KEYS,
  getFloatingOverlaySettings,
  getSettingsValuesFromNativeChange,
  type FloatingOverlaySettings,
} from "./settings";
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
import { type RenderLabelContext, SegmentKeyUtils } from "~/stt/live-segment";
import { buildSpeakerResolutionInput } from "~/stt/useResolvedSpeakerSegments";

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
    const speakerLabels: FloatingSpeakerLabels = new Map();
    const windowSynchronizer = createFloatingMeetingWindowSynchronizer(
      (state) => {
        useDictationStatus.setState({
          presentedOwner: state?.dictation?.sessionId ?? null,
        });
      },
    );
    let unsubscribeMeetingData: (() => Promise<void>) | null = null;
    const unlisteners: Array<() => void> = [];

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
                speakerLabels,
              )
            : null,
      );
    };
    refreshSettingsRef.current = refreshCurrentRouteState;

    const resolveSpeakers = createFloatingSpeakerResolver(
      speakerLabels,
      () => meetingData,
      () => refreshCurrentRouteState(true),
      () => cancelled,
    );

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
    refreshCurrentRouteState();
    const unsubscribeDictation = useDictationStatus.subscribe(() =>
      refreshCurrentRouteState(),
    );

    const unsubscribe = listenerStore.subscribe((state, previousState) => {
      if (!haveFloatingRouteInputsChanged(state, previousState)) {
        return;
      }

      refreshCurrentRouteState(
        state.liveSegments !== previousState.liveSegments ||
          state.live.sessionId !== previousState.live.sessionId,
      );
      if (
        state.liveSegments !== previousState.liveSegments ||
        state.live.sessionId !== previousState.live.sessionId
      ) {
        resolveSpeakers(state);
      }
    });

    void subscribeMeetingFloatData(
      (nextData) => {
        meetingData = nextData;
        refreshCurrentRouteState(true);
        resolveSpeakers(listenerStore.getState());
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

export function createFloatingSpeakerResolver(
  speakerLabels: FloatingSpeakerLabels,
  getMeetingData: () => MeetingFloatData,
  onUpdate: () => void,
  isCancelled: () => boolean = () => false,
): (state: ListenerState) => void {
  let sessionId: string | null = null;
  let counter = 0;

  return (state: ListenerState) => {
    if (sessionId !== state.live.sessionId) {
      sessionId = state.live.sessionId;
      speakerLabels.clear();
    }
    if (!sessionId || state.liveSegments.length === 0) return;

    const meetingData = getMeetingData();
    const startedAt = meetingData.sessions[sessionId]?.startedAtMs;
    if (!startedAt) return;
    const input = buildSpeakerResolutionInput(
      state.liveSegments,
      createMeetingFloatRenderRequest(meetingData, sessionId, startedAt),
    );
    if (!input) return;

    const requestId = ++counter;
    void (async () => {
      const result = await transcriptCommands.renderTranscriptSegments(input);
      if (isCancelled() || requestId !== counter || result.status !== "ok") {
        return;
      }
      const next = new Map<string, { label: string; humanId?: string }>();
      for (const segment of result.data) {
        const key = SegmentKeyUtils.serialize(segment.key);
        if (next.has(key)) continue;
        const label =
          segment.provisional_speaker?.name || segment.speaker_label;
        if (!label) continue;
        next.set(key, {
          label,
          humanId:
            segment.key.speaker_human_id ??
            segment.provisional_speaker?.human_id ??
            undefined,
        });
      }
      if (!areSpeakerLabelsEqual(speakerLabels, next)) {
        speakerLabels.clear();
        for (const [key, value] of next) speakerLabels.set(key, value);
        onUpdate();
      }
    })();
  };
}

function areSpeakerLabelsEqual(
  a: FloatingSpeakerLabels,
  b: Map<string, { label: string; humanId?: string }>,
) {
  if (a.size !== b.size) return false;
  for (const [key, value] of a) {
    const other = b.get(key);
    if (
      !other ||
      other.label !== value.label ||
      other.humanId !== value.humanId
    ) {
      return false;
    }
  }
  return true;
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
