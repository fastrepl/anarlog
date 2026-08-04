import { useRef } from "react";
import { useStore } from "zustand";

import {
  commands as detectCommands,
  events as detectEvents,
} from "@anlg/plugin-detect";
import { commands as notificationCommands } from "@anlg/plugin-notification";

import {
  AUTO_STOP_CONFIRM_DELAY_MS,
  getAutoStopActiveCheckAppIds,
  getAutoStopCandidateAppIds,
  getNetworkInterruptionDeadlineMs,
  shouldPromptBeforeAutoStopping,
  showMeetingEndedPrompt,
} from "./auto-stop";
import {
  getBrowserMeetingPlatform,
  getIgnorableApps,
  getIgnoreAppsFooterText,
  getNotificationAppName,
  getNotificationDisplayApp,
  getNotificationDisplayApps,
  getNotificationIconForDetectedApps,
  getNotificationIconForDisplayApp,
} from "./meeting-apps";

import {
  getNearbyCalendarEvents,
  type NearbyCalendarEvent,
} from "~/calendar/queries";
import { useConfigValue } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import type { ListenerStore } from "~/store/zustand/listener";

const MAX_TIMEOUT_DELAY_MS = 2_147_483_647;

type NearbyEvent = NearbyCalendarEvent;
type PendingAutoStop = {
  timeout?: ReturnType<typeof setTimeout>;
  requireMicSnapshot: boolean;
  sessionId: string | null;
  networkInterrupted: boolean;
};

function getMicDetectedNotificationTitle(event: NearbyEvent | null): string {
  if (!event) {
    return "Are you in a meeting?";
  }

  if (event.participantNames.length === 1) {
    return `Are you talking to ${event.participantNames[0]} right now?`;
  }

  if (event.participantNames.length === 2) {
    return `Are you talking to ${event.participantNames[0]} and ${event.participantNames[1]} right now?`;
  }

  return `Are you in ${event.title} right now?`;
}

export const useHandleDetectEvents = (store: ListenerStore) => {
  const stop = useStore(store, (state) => state.stop);
  const setMuted = useStore(store, (state) => state.setMuted);
  const autoStopMeetings = useConfigValue("auto_stop_meetings");
  const notificationDetect = useConfigValue("notification_detect");

  const autoStopMeetingsRef = useRef(autoStopMeetings);
  autoStopMeetingsRef.current = autoStopMeetings;
  const notificationDetectRef = useRef(notificationDetect);
  notificationDetectRef.current = notificationDetect;
  const isOnlineRef = useRef(true);
  const pendingAutoStopRef = useRef<PendingAutoStop | null>(null);
  const pendingMicDetectedPromptRef = useRef(false);

  useMountEffect(() => {
    let unlistenDetect: (() => void) | undefined;
    let cancelled = false;
    isOnlineRef.current = navigator.onLine;
    const clearNotificationsIfActive = () => {
      if (store.getState().live.status === "active") {
        void notificationCommands.clearNotifications();
      }
    };
    clearNotificationsIfActive();
    const unsubscribeStore = store.subscribe((state, previousState) => {
      if (
        state.live.status === "active" &&
        previousState.live.status !== "active"
      ) {
        void notificationCommands.clearNotifications();
      }
    });
    const clearPendingAutoStop = () => {
      if (pendingAutoStopRef.current) {
        if (pendingAutoStopRef.current.timeout) {
          clearTimeout(pendingAutoStopRef.current.timeout);
        }
        pendingAutoStopRef.current = null;
      }
    };
    const shouldCaptureMicDetectedTriggerApps = () => {
      const live = store.getState().live;
      return (
        live.status === "active" ||
        (live.status === "inactive" && live.loading && !!live.sessionId)
      );
    };
    const captureTriggerAppIds = (appIds: string[]) => {
      if (appIds.length === 0) {
        return;
      }

      const currentTrigger = store.getState().live.triggerAppIds ?? [];
      if (appIds.some((id) => currentTrigger.includes(id))) {
        clearPendingAutoStop();
      }
      store
        .getState()
        .setTriggerAppIds([...new Set([...currentTrigger, ...appIds])]);
    };

    function scheduleAutoStop(
      delayMs: number,
      candidateAppIds: string[],
      stoppedApps: { id: string; name: string }[],
      requireMicSnapshot: boolean,
      sessionId: string | null,
      networkInterrupted: boolean,
    ) {
      clearPendingAutoStop();

      const pending: PendingAutoStop = {
        requireMicSnapshot,
        sessionId,
        networkInterrupted,
      };
      pending.timeout = setTimeout(
        () => {
          void confirmAutoStop(candidateAppIds, stoppedApps, pending).finally(
            () => {
              if (pendingAutoStopRef.current === pending) {
                pendingAutoStopRef.current = null;
              }
            },
          );
        },
        Math.min(Math.max(delayMs, 0), MAX_TIMEOUT_DELAY_MS),
      );
      pendingAutoStopRef.current = pending;
    }

    async function confirmAutoStop(
      candidateAppIds: string[],
      stoppedApps: { id: string; name: string }[],
      pending: PendingAutoStop,
    ) {
      const live = store.getState().live;
      if (
        pendingAutoStopRef.current !== pending ||
        live.status !== "active" ||
        live.sessionId !== pending.sessionId
      ) {
        return;
      }

      const currentTrigger = live.triggerAppIds;
      if (
        !currentTrigger ||
        !candidateAppIds.some((id) => currentTrigger.includes(id))
      ) {
        return;
      }

      const activeCheckAppIds = getAutoStopActiveCheckAppIds(
        currentTrigger,
        candidateAppIds,
      );
      const hasUnreliableActiveCheckApp = activeCheckAppIds.some(
        (id) => !candidateAppIds.includes(id),
      );
      const result = await detectCommands.listMicUsingApplications();
      if (result.status === "ok") {
        const activeAppIds = new Set(result.data.map((app) => app.id));
        if (activeCheckAppIds.some((id) => activeAppIds.has(id))) {
          return;
        }
      } else if (pending.requireMicSnapshot || hasUnreliableActiveCheckApp) {
        return;
      }

      if (pendingAutoStopRef.current !== pending) {
        return;
      }

      if (pending.networkInterrupted || !isOnlineRef.current) {
        const deadlineMs = await getNetworkInterruptionDeadlineMs({
          sessionId: pending.sessionId,
          nowMs: Date.now(),
        });
        if (pendingAutoStopRef.current !== pending) {
          return;
        }
        if (deadlineMs) {
          scheduleAutoStop(
            deadlineMs - Date.now(),
            candidateAppIds,
            stoppedApps,
            pending.requireMicSnapshot,
            pending.sessionId,
            true,
          );
          return;
        }
      }

      const shouldPrompt = await shouldPromptBeforeAutoStopping({
        appIds: candidateAppIds,
        sessionId: pending.sessionId,
        nowMs: Date.now(),
      });
      if (pendingAutoStopRef.current !== pending) {
        return;
      }
      if (shouldPrompt) {
        if (pending.sessionId) {
          await showMeetingEndedPrompt({
            sessionId: pending.sessionId,
            stoppedTriggerAppIds: candidateAppIds,
            stoppedApps,
          });
        }
        return;
      }

      const currentLive = store.getState().live;
      if (
        pendingAutoStopRef.current !== pending ||
        currentLive.status !== "active" ||
        currentLive.sessionId !== pending.sessionId
      ) {
        return;
      }

      stop();
    }

    const handleOffline = () => {
      isOnlineRef.current = false;
      if (pendingAutoStopRef.current) {
        pendingAutoStopRef.current.networkInterrupted = true;
      }
    };
    const handleOnline = () => {
      isOnlineRef.current = true;
    };
    window.addEventListener("offline", handleOffline);
    window.addEventListener("online", handleOnline);

    detectEvents.detectEvent
      .listen(({ payload }) => {
        if (payload.type === "micDetected") {
          const ignorableApps = getIgnorableApps(payload.apps);
          const appIds = ignorableApps.map((app) => app.id);

          if (shouldCaptureMicDetectedTriggerApps()) {
            captureTriggerAppIds(appIds);
            return;
          }

          if (!notificationDetectRef.current) {
            return;
          }

          if (pendingMicDetectedPromptRef.current) {
            return;
          }
          pendingMicDetectedPromptRef.current = true;

          void (async () => {
            try {
              const nearbyEvents = await getNearbyCalendarEvents(
                Date.now(),
                15 * 60 * 1000,
              );
              const nearbyEvent = nearbyEvents[0] ?? null;
              const browserMeetingPlatform = getBrowserMeetingPlatform(
                payload.apps,
                nearbyEvent,
              );
              const displayApps = getNotificationDisplayApps(
                payload.apps,
                browserMeetingPlatform,
              );
              const displayIgnorableApps = ignorableApps.map((app) =>
                getNotificationDisplayApp(app, browserMeetingPlatform),
              );

              const footerIcon =
                displayIgnorableApps.length > 0
                  ? await getNotificationIconForDisplayApp(
                      displayIgnorableApps[0]!,
                      browserMeetingPlatform,
                    )
                  : null;
              const notificationIcon = await getNotificationIconForDetectedApps(
                payload.apps,
                browserMeetingPlatform,
              );
              const footer =
                displayIgnorableApps.length > 0
                  ? {
                      text: getIgnoreAppsFooterText(displayIgnorableApps),
                      actionLabel: "Yes",
                      icon: footerIcon,
                    }
                  : null;

              if (shouldCaptureMicDetectedTriggerApps()) {
                captureTriggerAppIds(appIds);
                return;
              }

              await notificationCommands.showNotification({
                key: payload.key,
                title: getMicDetectedNotificationTitle(nearbyEvent),
                message: "",
                timeout: { secs: 15, nanos: 0 },
                source: {
                  type: "mic_detected",
                  app_names: displayApps.map((app) =>
                    getNotificationAppName(app),
                  ),
                  app_ids: appIds,
                  event_ids: nearbyEvent ? [nearbyEvent.id] : [],
                },
                start_time: null,
                participants: null,
                event_details: null,
                action_label: "Yes",
                action_variant: null,
                options: null,
                footer,
                icon: notificationIcon,
              });
            } finally {
              pendingMicDetectedPromptRef.current = false;
            }
          })();
        } else if (payload.type === "micStopped") {
          const autoStopEnabled = autoStopMeetingsRef.current !== false;
          if (!autoStopEnabled) {
            return;
          }

          const trigger = store.getState().live.triggerAppIds;
          const stoppedTriggerAppIds =
            trigger?.filter((id) =>
              payload.apps.some((app) => app.id === id),
            ) ?? [];
          const candidateAppIds = getAutoStopCandidateAppIds(
            trigger,
            payload.apps,
          );
          if (candidateAppIds.length > 0) {
            const requireMicSnapshot = stoppedTriggerAppIds.length === 0;
            if (
              pendingAutoStopRef.current &&
              !pendingAutoStopRef.current.requireMicSnapshot &&
              requireMicSnapshot
            ) {
              return;
            }

            scheduleAutoStop(
              AUTO_STOP_CONFIRM_DELAY_MS,
              candidateAppIds,
              payload.apps,
              requireMicSnapshot,
              store.getState().live.sessionId,
              !isOnlineRef.current,
            );
          }
        } else if (payload.type === "sleepStateChanged") {
          if (payload.value) {
            clearPendingAutoStop();
            stop();
          }
        } else if (payload.type === "micMuted") {
          setMuted(payload.value);
        }
      })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlistenDetect = fn;
        }
      })
      .catch((err) => {
        console.error("Failed to setup detect event listener:", err);
      });

    return () => {
      cancelled = true;
      clearPendingAutoStop();
      window.removeEventListener("offline", handleOffline);
      window.removeEventListener("online", handleOnline);
      unsubscribeStore();
      unlistenDetect?.();
    };
  });
};
