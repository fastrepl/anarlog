import { type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

import { events as notificationEvents } from "@hypr/plugin-notification";
import {
  commands as updaterCommands,
  events as updaterEvents,
} from "@hypr/plugin-updater2";
import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import { useMountEffect } from "~/shared/hooks/useMountEffect";
import * as main from "~/store/tinybase/store/main";
import {
  createSession,
  getOrCreateSessionForEventId,
} from "~/store/tinybase/store/sessions";
import * as settings from "~/store/tinybase/store/settings";
import { listenerStore } from "~/store/zustand/listener/instance";
import { useTabs } from "~/store/zustand/tabs";
import { parseAutoStopEndedNotificationKey } from "~/stt/auto-stop-notification";
import { parseBatchCompletedNotificationKey } from "~/stt/batch-completed-notification";
import {
  getLiveTranscriptionConfig,
  getTranscriptionLanguages,
} from "~/stt/capabilities";

type MainStore = NonNullable<ReturnType<typeof main.UI.useStore>>;
type SettingsStore = NonNullable<ReturnType<typeof settings.UI.useStore>>;

const LIVE_CAPTURE_CONFIG_DEBOUNCE_MS = 750;

function parseIgnoredPlatforms(value: unknown) {
  if (typeof value !== "string") {
    return [];
  }

  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed)
      ? parsed.filter(
          (bundleId): bundleId is string => typeof bundleId === "string",
        )
      : [];
  } catch {
    return [];
  }
}

function shouldAutoStartNotificationSession(
  store: MainStore,
  eventId: string | null,
  triggerAppIds: string[] | null,
): boolean {
  if (triggerAppIds && triggerAppIds.length > 0) {
    return true;
  }

  if (!eventId) {
    return true;
  }

  const startedAt = store.getRow("events", eventId)?.started_at;
  if (!startedAt) {
    return false;
  }

  const startTime = new Date(String(startedAt)).getTime();
  return !Number.isNaN(startTime) && startTime <= Date.now();
}

function handleAutoStopEndedNotification(
  type: "notification_confirm" | "notification_accept" | "notification_timeout",
  key: string,
): boolean {
  const sessionId = parseAutoStopEndedNotificationKey(key);
  if (!sessionId) {
    return false;
  }

  if (type === "notification_confirm") {
    return true;
  }

  const listenerState = listenerStore.getState();
  if (
    listenerState.live.status === "active" &&
    listenerState.live.sessionId === sessionId
  ) {
    listenerState.stop();
  }

  return true;
}

function getSessionParticipantHumanIds(store: MainStore, sessionId: string) {
  const seen = new Set<string>();
  const participantHumanIds: string[] = [];

  store.forEachRow("mapping_session_participant", (mappingId, _forEachCell) => {
    const sid = store.getCell(
      "mapping_session_participant",
      mappingId,
      "session_id",
    );
    if (sid !== sessionId) return;

    const humanId = store.getCell(
      "mapping_session_participant",
      mappingId,
      "human_id",
    );
    if (typeof humanId !== "string" || !humanId || seen.has(humanId)) {
      return;
    }

    seen.add(humanId);
    participantHumanIds.push(humanId);
  });

  return participantHumanIds;
}

function createCaptureConfigSignature(config: {
  session_id: string;
  languages: string[];
  participant_human_ids: string[];
  self_human_id: string | null;
}) {
  return JSON.stringify(config);
}

function parseStringArray(value: unknown, fallback: string[]) {
  if (Array.isArray(value)) {
    return value.filter((item): item is string => typeof item === "string");
  }

  if (typeof value !== "string") {
    return fallback;
  }

  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed)
      ? parsed.filter((item): item is string => typeof item === "string")
      : fallback;
  } catch {
    return fallback;
  }
}

function getSettingsDefault(key: "ai_language" | "spoken_languages") {
  const mapping = settings.SETTINGS_MAPPING?.values[key];
  return mapping && "default" in mapping ? mapping.default : undefined;
}

function getLiveConfigLanguages(settingsStore: SettingsStore) {
  const aiLanguageValue = settingsStore.getValue("ai_language");
  const aiLanguage =
    typeof aiLanguageValue === "string"
      ? aiLanguageValue
      : typeof getSettingsDefault("ai_language") === "string"
        ? getSettingsDefault("ai_language")
        : undefined;

  return getTranscriptionLanguages(
    aiLanguage,
    parseStringArray(
      settingsStore.getValue("spoken_languages"),
      parseStringArray(getSettingsDefault("spoken_languages"), []),
    ),
  );
}

function LiveCaptureConfigSync() {
  const store = main.UI.useStore(main.STORE_ID);
  const settingsStore = settings.UI.useStore(settings.STORE_ID);

  if (!store || !settingsStore) {
    return null;
  }

  return (
    <LiveCaptureConfigSyncReady
      settingsStore={settingsStore as SettingsStore}
      store={store as MainStore}
    />
  );
}

function LiveCaptureConfigSyncReady({
  store,
  settingsStore,
}: {
  store: MainStore;
  settingsStore: SettingsStore;
}) {
  useMountEffect(() => {
    let timeoutId: ReturnType<typeof setTimeout> | null = null;
    let lastSignature: string | null = null;

    const pushConfig = async () => {
      const live = listenerStore.getState().live;
      if (live.status !== "active" || !live.sessionId) {
        return;
      }

      const languages = getLiveConfigLanguages(settingsStore);
      const provider = settingsStore.getValue("current_stt_provider");
      const model = settingsStore.getValue("current_stt_model");
      const liveConfig = await getLiveTranscriptionConfig({
        provider: typeof provider === "string" ? provider : undefined,
        model: typeof model === "string" ? model : undefined,
        languages,
      });

      if (liveConfig.transcriptionMode === "batch") {
        return;
      }

      const selfHumanId = store.getValue("user_id");
      const nextConfig = {
        session_id: live.sessionId,
        languages: liveConfig.languages,
        participant_human_ids: getSessionParticipantHumanIds(
          store,
          live.sessionId,
        ),
        self_human_id: typeof selfHumanId === "string" ? selfHumanId : null,
      };
      const signature = createCaptureConfigSignature(nextConfig);
      if (signature === lastSignature) {
        return;
      }

      lastSignature = signature;
      await listenerStore.getState().updateCaptureConfig(nextConfig);
    };

    const schedulePush = () => {
      if (timeoutId) {
        clearTimeout(timeoutId);
      }

      timeoutId = setTimeout(() => {
        void pushConfig().catch((error) => {
          console.error(
            "[listener] failed to update live capture config",
            error,
          );
        });
      }, LIVE_CAPTURE_CONFIG_DEBOUNCE_MS);
    };

    const mainListenerIds = [
      store.addTableListener("mapping_session_participant", schedulePush),
    ];
    const settingsListenerIds = [
      settingsStore.addValueListener("ai_language", schedulePush),
      settingsStore.addValueListener("spoken_languages", schedulePush),
      settingsStore.addValueListener("current_stt_provider", schedulePush),
      settingsStore.addValueListener("current_stt_model", schedulePush),
    ];

    schedulePush();

    return () => {
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
      for (const listenerId of mainListenerIds) {
        store.delListener(listenerId);
      }
      for (const listenerId of settingsListenerIds) {
        settingsStore.delListener(listenerId);
      }
    };
  });

  return null;
}

function useUpdaterEvents() {
  const openNew = useTabs((state) => state.openNew);

  useEffect(() => {
    if (getCurrentWebviewWindowLabel() !== "main") {
      return;
    }

    let unlisten: UnlistenFn | null = null;

    void updaterEvents.updatedEvent
      .listen(({ payload: { previous, current } }) => {
        openNew({
          type: "changelog",
          state: { previous, current },
        });
      })
      .then((f) => {
        unlisten = f;
        updaterCommands.maybeEmitUpdated();
      });

    return () => {
      unlisten?.();
    };
  }, [openNew]);
}

function useNotificationEvents() {
  const store = main.UI.useStore(main.STORE_ID);
  const settingsStore = settings.UI.useStore(settings.STORE_ID);
  const openNew = useTabs((state) => state.openNew);
  const pendingAutoStart = useRef<{
    eventId: string | null;
    triggerAppIds: string[] | null;
  } | null>(null);
  const storeRef = useRef(store);
  const settingsStoreRef = useRef(settingsStore);
  const openNewRef = useRef(openNew);

  useEffect(() => {
    storeRef.current = store;
    settingsStoreRef.current = settingsStore;
    openNewRef.current = openNew;
  }, [store, settingsStore, openNew]);

  useEffect(() => {
    if (pendingAutoStart.current && store) {
      const { eventId, triggerAppIds } = pendingAutoStart.current;
      pendingAutoStart.current = null;
      const sessionId = eventId
        ? getOrCreateSessionForEventId(store, eventId)
        : createSession(store);

      if (triggerAppIds && triggerAppIds.length > 0) {
        listenerStore.getState().setTriggerAppIds(triggerAppIds);
      }
      const autoStart = shouldAutoStartNotificationSession(
        store,
        eventId,
        triggerAppIds,
      );

      openNew({
        type: "sessions",
        id: sessionId,
        state: { view: null, autoStart: autoStart ? true : null },
      });
    }
  }, [store, openNew]);

  useEffect(() => {
    if (getCurrentWebviewWindowLabel() !== "main") {
      return;
    }

    let unlisten: UnlistenFn | null = null;
    let cancelled = false;

    void notificationEvents.notificationEvent
      .listen(({ payload }) => {
        if (
          payload.type === "notification_confirm" ||
          payload.type === "notification_accept" ||
          payload.type === "notification_timeout"
        ) {
          if (handleAutoStopEndedNotification(payload.type, payload.key)) {
            return;
          }

          if (payload.type === "notification_timeout") {
            return;
          }

          const eventId =
            payload.source?.type === "calendar_event"
              ? payload.source.event_id
              : null;
          const sourceSessionId =
            payload.source?.type === "session"
              ? payload.source.session_id
              : parseBatchCompletedNotificationKey(payload.key);
          const triggerAppIds =
            payload.source?.type === "mic_detected"
              ? (payload.source.app_ids ?? null)
              : null;
          const currentStore = storeRef.current;
          if (sourceSessionId) {
            openNewRef.current({
              type: "sessions",
              id: sourceSessionId,
              state: { view: null, autoStart: null },
            });
            return;
          }

          if (!currentStore) {
            pendingAutoStart.current = { eventId, triggerAppIds };
            return;
          }
          const sessionId = eventId
            ? getOrCreateSessionForEventId(currentStore, eventId)
            : createSession(currentStore);

          if (triggerAppIds && triggerAppIds.length > 0) {
            listenerStore.getState().setTriggerAppIds(triggerAppIds);
          }
          const autoStart = shouldAutoStartNotificationSession(
            currentStore,
            eventId,
            triggerAppIds,
          );

          openNewRef.current({
            type: "sessions",
            id: sessionId,
            state: { view: null, autoStart: autoStart ? true : null },
          });
        } else if (payload.type === "notification_option_selected") {
          const currentStore = storeRef.current;
          if (!currentStore) return;

          const selectedIndex = payload.selected_index;
          const eventIds =
            payload.source?.type === "mic_detected"
              ? (payload.source.event_ids ?? [])
              : [];

          const sessionId =
            selectedIndex < eventIds.length
              ? getOrCreateSessionForEventId(
                  currentStore,
                  eventIds[selectedIndex],
                )
              : createSession(currentStore);

          if (payload.source?.type === "mic_detected") {
            const triggerAppIds = payload.source.app_ids ?? [];
            listenerStore
              .getState()
              .setTriggerAppIds(
                triggerAppIds.length > 0 ? triggerAppIds : null,
              );
          }

          openNewRef.current({
            type: "sessions",
            id: sessionId,
            state: { view: null, autoStart: true },
          });
        } else if (payload.type === "notification_footer_action") {
          if (payload.source?.type !== "mic_detected") {
            return;
          }

          const currentSettingsStore = settingsStoreRef.current;
          if (!currentSettingsStore) {
            return;
          }

          const appIds = payload.source.app_ids ?? [];
          if (appIds.length === 0) {
            return;
          }

          const ignoredPlatforms = parseIgnoredPlatforms(
            currentSettingsStore.getValue("ignored_platforms"),
          );
          const nextIgnoredPlatforms = [
            ...new Set([...ignoredPlatforms, ...appIds]),
          ];

          if (nextIgnoredPlatforms.length === ignoredPlatforms.length) {
            return;
          }

          currentSettingsStore.setValue(
            "ignored_platforms",
            JSON.stringify(nextIgnoredPlatforms),
          );
        }
      })
      .then((f) => {
        if (cancelled) {
          f();
        } else {
          unlisten = f;
        }
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);
}

export function EventListeners() {
  return (
    <>
      <EventListenersInner />
      <LiveCaptureConfigSync />
    </>
  );
}

function EventListenersInner() {
  useUpdaterEvents();
  useNotificationEvents();

  return null;
}
