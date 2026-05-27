import React, { createContext, useContext, useEffect, useRef } from "react";
import { useStore } from "zustand";
import { useShallow } from "zustand/shallow";

import {
  commands as detectCommands,
  events as detectEvents,
} from "@hypr/plugin-detect";
import {
  commands as notificationCommands,
  type NotificationIcon,
} from "@hypr/plugin-notification";

import { createAutoStopEndedNotificationKey } from "./auto-stop-notification";

import { getSessionEventById } from "~/session/utils";
import * as main from "~/store/tinybase/store/main";
import * as settings from "~/store/tinybase/store/settings";
import {
  createListenerStore,
  type ListenerStore,
} from "~/store/zustand/listener";

const ListenerContext = createContext<ListenerStore | null>(null);
export const AUTO_STOP_CONFIRM_DELAY_MS = 5_000;
export const AUTO_STOP_CALENDAR_EARLY_END_THRESHOLD_MS = 3 * 60_000;
export const AUTO_STOP_CALENDAR_EARLY_START_BUFFER_MS = 5 * 60_000;

const BROWSER_MEETING_APP_IDS = new Set([
  "app.zen-browser.zen",
  "com.apple.Safari",
  "com.brave.Browser",
  "com.google.Chrome",
  "com.google.Chrome.canary",
  "com.microsoft.edgemac",
  "com.microsoft.edgemac.Canary",
  "com.operasoftware.Opera",
  "com.vivaldi.Vivaldi",
  "company.thebrowser.Browser",
  "org.mozilla.firefox",
]);

type MainStore = NonNullable<ReturnType<typeof main.UI.useStore>>;

function getIgnorableApps(apps: { id: string; name: string }[]) {
  const seen = new Set<string>();

  return apps.filter((app) => {
    if (!app.id || app.id.startsWith("pid:") || seen.has(app.id)) {
      return false;
    }

    seen.add(app.id);
    return true;
  });
}

function getNotificationIconForAppId(appId: string): NotificationIcon | null {
  if (!appId || appId.startsWith("pid:")) {
    return null;
  }

  if (appId.startsWith("/") || appId.startsWith("~/")) {
    return { type: "path", path: appId };
  }

  return { type: "bundle_id", bundle_id: appId };
}

function getIgnoreAppsFooterText(apps: { name: string }[]) {
  const firstName = apps[0]?.name.trim();

  if (apps.length === 1) {
    return firstName ? `Ignore ${firstName}?` : "Ignore this app?";
  }

  if (!firstName) {
    return "Ignore these apps?";
  }

  const secondName = apps[1]?.name.trim();
  if (apps.length === 2 && secondName) {
    return `Ignore ${firstName} and ${secondName}?`;
  }

  const otherAppCount = apps.length - 1;
  return `Ignore ${firstName} and ${otherAppCount} other app${otherAppCount === 1 ? "" : "s"}?`;
}

function parseEventTimeMs(value: string | undefined): number | null {
  if (!value) {
    return null;
  }

  const ms = new Date(value).getTime();
  return Number.isNaN(ms) ? null : ms;
}

function shouldPromptBeforeAutoStopping({
  appIds,
  tinybaseStore,
  sessionId,
  nowMs,
}: {
  appIds: string[];
  tinybaseStore: MainStore | null | undefined;
  sessionId: string | null;
  nowMs: number;
}) {
  if (!appIds.some((id) => BROWSER_MEETING_APP_IDS.has(id))) {
    return false;
  }

  if (!tinybaseStore || !sessionId) {
    return false;
  }

  const event = getSessionEventById(tinybaseStore, sessionId);
  if (!event || event.is_all_day) {
    return false;
  }

  const endMs = parseEventTimeMs(event.ended_at);
  if (!endMs) {
    return false;
  }

  const startMs = parseEventTimeMs(event.started_at);
  if (startMs && nowMs < startMs - AUTO_STOP_CALENDAR_EARLY_START_BUFFER_MS) {
    return false;
  }

  return nowMs < endMs - AUTO_STOP_CALENDAR_EARLY_END_THRESHOLD_MS;
}

function getPrimaryStoppedApp(
  stoppedTriggerAppIds: string[],
  stoppedApps: { id: string; name: string }[],
) {
  return (
    stoppedApps.find(
      (app) =>
        stoppedTriggerAppIds.includes(app.id) &&
        BROWSER_MEETING_APP_IDS.has(app.id),
    ) ??
    stoppedApps.find((app) => stoppedTriggerAppIds.includes(app.id)) ??
    null
  );
}

function getStoppedAppLabel(app: { name: string } | null) {
  const name = app?.name.trim();
  return name || "The meeting app";
}

function showMeetingEndedPrompt({
  sessionId,
  stoppedTriggerAppIds,
  stoppedApps,
}: {
  sessionId: string;
  stoppedTriggerAppIds: string[];
  stoppedApps: { id: string; name: string }[];
}) {
  const app = getPrimaryStoppedApp(stoppedTriggerAppIds, stoppedApps);

  void notificationCommands.showNotification({
    key: createAutoStopEndedNotificationKey(sessionId),
    title: "Did your meeting end?",
    message: `${getStoppedAppLabel(app)} stopped using the microphone before the scheduled end time.`,
    timeout: { secs: 60, nanos: 0 },
    source: null,
    start_time: null,
    participants: null,
    event_details: null,
    action_label: "Stop recording",
    options: null,
    footer: null,
    icon: app ? getNotificationIconForAppId(app.id) : null,
  });
}

export const ListenerProvider = ({
  children,
  store,
}: {
  children: React.ReactNode;
  store: ListenerStore;
}) => {
  useHandleDetectEvents(store);

  const storeRef = useRef<ListenerStore | null>(null);
  if (!storeRef.current) {
    storeRef.current = store;
  }

  return (
    <ListenerContext.Provider value={storeRef.current}>
      {children}
    </ListenerContext.Provider>
  );
};

export const useListener = <T,>(
  selector: Parameters<
    typeof useStore<ReturnType<typeof createListenerStore>, T>
  >[1],
) => {
  const store = useContext(ListenerContext);

  if (!store) {
    throw new Error("'useListener' must be used within a 'ListenerProvider'");
  }

  return useStore(store, useShallow(selector));
};

function getNearbyEvents(
  tinybaseStore: NonNullable<ReturnType<typeof main.UI.useStore>>,
): { id: string; title: string }[] {
  const now = Date.now();
  const windowMs = 15 * 60 * 1000;
  const results: { id: string; title: string; startedAt: number }[] = [];

  tinybaseStore.forEachRow("events", (eventId, _forEachCell) => {
    const event = tinybaseStore.getRow("events", eventId);
    if (!event?.started_at) return;
    if (event.is_all_day) return;

    const startTime = new Date(String(event.started_at)).getTime();
    if (isNaN(startTime)) return;

    if (Math.abs(startTime - now) <= windowMs) {
      results.push({
        id: eventId,
        title: String(event.title || "Untitled Event"),
        startedAt: startTime,
      });
    }
  });

  results.sort((a, b) => a.startedAt - b.startedAt);
  return results.map(({ id, title }) => ({ id, title }));
}

const useHandleDetectEvents = (store: ListenerStore) => {
  const stop = useStore(store, (state) => state.stop);
  const setMuted = useStore(store, (state) => state.setMuted);
  const tinybaseStore = main.UI.useStore(main.STORE_ID);
  const settingsStore = settings.UI.useStore(settings.STORE_ID);

  const tinybaseStoreRef = useRef(tinybaseStore);
  const settingsStoreRef = useRef(settingsStore);
  const pendingAutoStopRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    tinybaseStoreRef.current = tinybaseStore;
    settingsStoreRef.current = settingsStore;
  }, [tinybaseStore, settingsStore]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    const clearPendingAutoStop = () => {
      if (pendingAutoStopRef.current) {
        clearTimeout(pendingAutoStopRef.current);
        pendingAutoStopRef.current = null;
      }
    };

    const confirmAutoStop = async (
      stoppedTriggerAppIds: string[],
      stoppedApps: { id: string; name: string }[],
    ) => {
      const live = store.getState().live;
      if (live.status !== "active") {
        return;
      }

      const currentTrigger = live.triggerAppIds;
      if (
        !currentTrigger ||
        !stoppedTriggerAppIds.some((id) => currentTrigger.includes(id))
      ) {
        return;
      }

      const result = await detectCommands.listMicUsingApplications();
      if (result.status === "ok") {
        const activeAppIds = new Set(result.data.map((app) => app.id));
        if (stoppedTriggerAppIds.some((id) => activeAppIds.has(id))) {
          return;
        }
      }

      const sessionId = store.getState().live.sessionId;
      if (
        shouldPromptBeforeAutoStopping({
          appIds: stoppedTriggerAppIds,
          tinybaseStore: tinybaseStoreRef.current,
          sessionId,
          nowMs: Date.now(),
        })
      ) {
        if (sessionId) {
          showMeetingEndedPrompt({
            sessionId,
            stoppedTriggerAppIds,
            stoppedApps,
          });
        }
        return;
      }

      stop();
    };

    detectEvents.detectEvent
      .listen(({ payload }) => {
        if (payload.type === "micDetected") {
          const ignorableApps = getIgnorableApps(payload.apps);
          const appIds = ignorableApps.map((app) => app.id);

          const live = store.getState().live;
          const shouldCaptureTriggerApps =
            live.status === "active" ||
            (live.status === "inactive" && live.loading && !!live.sessionId);

          if (shouldCaptureTriggerApps) {
            if (appIds.length > 0) {
              const currentTrigger = store.getState().live.triggerAppIds ?? [];
              if (appIds.some((id) => currentTrigger.includes(id))) {
                clearPendingAutoStop();
              }
              store
                .getState()
                .setTriggerAppIds([...new Set([...currentTrigger, ...appIds])]);
            }
            return;
          }

          const currentTinybaseStore = tinybaseStoreRef.current;
          const nearbyEvents = currentTinybaseStore
            ? getNearbyEvents(currentTinybaseStore)
            : [];

          const options =
            nearbyEvents.length > 0 ? nearbyEvents.map((e) => e.title) : null;
          const footer =
            ignorableApps.length > 0
              ? {
                  text: getIgnoreAppsFooterText(ignorableApps),
                  actionLabel: "Yes",
                  icon: getNotificationIconForAppId(ignorableApps[0]!.id),
                }
              : null;

          void notificationCommands.showNotification({
            key: payload.key,
            title: "Are you in a meeting?",
            message: "",
            timeout: { secs: 15, nanos: 0 },
            source: {
              type: "mic_detected",
              app_names: payload.apps.map((a) => a.name),
              app_ids: appIds,
              event_ids: nearbyEvents.map((e) => e.id),
            },
            start_time: null,
            participants: null,
            event_details: null,
            action_label: null,
            options,
            footer,
            icon: null,
          });
        } else if (payload.type === "micStopped") {
          const autoStopEnabled =
            settingsStoreRef.current?.getValue("auto_stop_meetings") !== false;
          if (!autoStopEnabled) {
            return;
          }

          const trigger = store.getState().live.triggerAppIds;
          const stoppedTriggerAppIds =
            trigger?.filter((id) =>
              payload.apps.some((app) => app.id === id),
            ) ?? [];
          if (stoppedTriggerAppIds.length > 0) {
            clearPendingAutoStop();

            pendingAutoStopRef.current = setTimeout(() => {
              pendingAutoStopRef.current = null;
              void confirmAutoStop(stoppedTriggerAppIds, payload.apps);
            }, AUTO_STOP_CONFIRM_DELAY_MS);
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
          unlisten = fn;
        }
      })
      .catch((err) => {
        console.error("Failed to setup detect event listener:", err);
      });

    return () => {
      cancelled = true;
      clearPendingAutoStop();
      unlisten?.();
    };
  }, [stop, setMuted, store]);
};
