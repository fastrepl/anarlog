import { useQuery } from "@tanstack/react-query";
import { useCallback, useRef, useState } from "react";

import { commands as notificationCommands } from "@hypr/plugin-notification";
import {
  commands as windowsCommands,
  events as windowsEvents,
  getCurrentWebviewWindowLabel,
  openUrlWithInstruction,
} from "@hypr/plugin-windows";

import { useBillingAccess } from "~/auth/billing";
import { TrialEndedDialog } from "~/billing/trial-ended-dialog";
import { TrialStartedDialog } from "~/billing/trial-started-dialog";
import { getLatestVersion } from "~/changelog";
import { useDevtoolsStore, useDevtoolsUserId } from "~/devtools-panel/hooks";
import { populateRecurringMeetingNotes } from "~/devtools-panel/recurring-notes";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import {
  type DevtoolsOtaPreviewStatus,
  useDevtoolsOtaPreview,
} from "~/store/zustand/devtools-ota-preview";
import {
  type DevtoolsToastPreview,
  useDevtoolsToastPreview,
} from "~/store/zustand/devtools-toast-preview";
import { showBatchCompletedNotification } from "~/store/zustand/listener/general-batch";
import { listenerStore } from "~/store/zustand/listener/instance";
import { useTabs } from "~/store/zustand/tabs";
import {
  AUTO_STOP_CONFIRM_TIMEOUT_SECONDS,
  createAutoStopEndedNotificationKey,
} from "~/stt/auto-stop-notification";
import { commands } from "~/types/tauri.gen";

const canResolveDevtoolsPanel = import.meta.env.MODE !== "test";

type DevtoolsPanelAction =
  | "navigation:onboarding"
  | "navigation:empty"
  | "navigation:changelog"
  | "instruction:sign-in"
  | "instruction:billing"
  | "instruction:integration"
  | `toasts:preview:${DevtoolsToastPreview}`
  | "toasts:preview:clear"
  | "toasts:reset-dismissed"
  | "ota:available"
  | "ota:downloading"
  | "ota:ready"
  | "ota:failed"
  | "ota:clear"
  | "notifications:calendar"
  | "notifications:mic-detected"
  | "notifications:mic-options"
  | "notifications:auto-stop"
  | "notifications:batch-done"
  | "notifications:clear"
  | "billing:trial-started"
  | "billing:trial-ended"
  | "notes:populate-recurring"
  | "countdown:note-20"
  | "countdown:note-60"
  | "countdown:note-290"
  | "countdown:zoom-20"
  | "countdown:zoom-60"
  | "error:trigger";

export function DevtoolsFloatingPanelHost() {
  const isMainWindow = getCurrentWebviewWindowLabel() === "main";
  const shouldShow = useShouldShowDevtoolsPanel(isMainWindow);

  if (!isMainWindow) {
    return null;
  }

  if (!shouldShow) {
    return <DevtoolsFloatingPanelDisabled />;
  }

  return <DevtoolsFloatingPanelSync />;
}

function useShouldShowDevtoolsPanel(isMainWindow: boolean) {
  const enabledQuery = useQuery({
    queryKey: ["devtools-panel", "enabled"],
    queryFn: commands.showDevtool,
    enabled: isMainWindow && canResolveDevtoolsPanel,
    staleTime: Infinity,
  });

  return enabledQuery.data ?? false;
}

function DevtoolsFloatingPanelDisabled() {
  useMountEffect(() => {
    void hideDevtoolsPanel();
  });

  return null;
}

function DevtoolsFloatingPanelSync() {
  const { dialogs, handleAction, shouldThrow } = useDevtoolsPanelActions();
  const actionHandlerRef = useRef(handleAction);
  actionHandlerRef.current = handleAction;

  useMountEffect(() => {
    let cancelled = false;
    let unlistenAction: (() => void) | undefined;

    void showDevtoolsPanel();

    windowsEvents.devtoolsPanelAction
      .listen(({ payload }) => {
        actionHandlerRef.current(payload.action);
      })
      .then((unlisten) => {
        if (cancelled) {
          unlisten();
          return;
        }

        unlistenAction = unlisten;
      });

    return () => {
      cancelled = true;
      unlistenAction?.();
      void hideDevtoolsPanel();
    };
  });

  if (shouldThrow) {
    throw new Error("Test error triggered from devtools");
  }

  return dialogs;
}

function useDevtoolsPanelActions() {
  const openNew = useTabs((s) => s.openNew);
  const store = useDevtoolsStore();
  const user_id = useDevtoolsUserId();
  const { trialDaysRemaining, upgradeToPro } = useBillingAccess();
  const showToastPreview = useDevtoolsToastPreview(
    (state) => state.showPreview,
  );
  const clearToastPreview = useDevtoolsToastPreview(
    (state) => state.clearPreview,
  );
  const showOtaPreview = useDevtoolsOtaPreview((state) => state.showPreview);
  const clearOtaPreview = useDevtoolsOtaPreview((state) => state.clearPreview);
  const [trialStartedOpen, setTrialStartedOpen] = useState(false);
  const [trialEndedOpen, setTrialEndedOpen] = useState(false);
  const [shouldThrow, setShouldThrow] = useState(false);

  const showMainWindow = useCallback(async () => {
    await windowsCommands.windowShow({ type: "main" });
  }, []);

  const isClassicMain =
    typeof window !== "undefined" &&
    (window.location.pathname === "/app/main" ||
      window.location.pathname.startsWith("/app/main/"));

  const showOnboarding = useCallback(async () => {
    await showMainWindow();
    openNew({ type: "onboarding" });
  }, [openNew, showMainWindow]);

  const showEmptyTab = useCallback(async () => {
    if (!isClassicMain) {
      return;
    }

    await showMainWindow();
    openNew({ type: "empty" });
  }, [isClassicMain, openNew, showMainWindow]);

  const showInstruction = useCallback((type: string) => {
    void openUrlWithInstruction(
      `https://example.com/${type}`,
      type,
      async () => ({ status: "ok" as const }),
    );
  }, []);

  const showChangelog = useCallback(() => {
    const latestVersion = getLatestVersion();
    if (!latestVersion) {
      return;
    }

    openNew({
      type: "changelog",
      state: { current: latestVersion, previous: null },
    });
  }, [openNew]);

  const showToastPreviewInMainWindow = useCallback(
    async (preview: DevtoolsToastPreview) => {
      await showMainWindow();
      showToastPreview(preview);
    },
    [showMainWindow, showToastPreview],
  );

  const showOtaPreviewInMainWindow = useCallback(
    async (preview: DevtoolsOtaPreviewStatus) => {
      await showMainWindow();
      showOtaPreview(preview);
    },
    [showMainWindow, showOtaPreview],
  );

  const populateRecurringNotes = useCallback(async () => {
    if (!store) {
      return;
    }

    const sessionId = populateRecurringMeetingNotes({ store, userId: user_id });
    await showMainWindow();
    openNew({ type: "sessions", id: sessionId });
  }, [openNew, showMainWindow, store, user_id]);

  const showCalendarNotification = useCallback(async () => {
    const eventId = `devtool-event-${crypto.randomUUID()}`;
    const startedAt = new Date(Date.now() + 5 * 60 * 1000);
    const endedAt = new Date(startedAt.getTime() + 30 * 60 * 1000);

    store?.setRow("events", eventId, {
      user_id: user_id ?? "",
      created_at: new Date().toISOString(),
      tracking_id_event: eventId,
      calendar_id: "devtool-calendar",
      title: "Devtool design sync",
      started_at: startedAt.toISOString(),
      ended_at: endedAt.toISOString(),
      location: "Conference Room",
      meeting_link: "https://zoom.us/j/1234567890",
      description: "Notification test event",
      note: "",
      recurrence_series_id: "",
      has_recurrence_rules: false,
      is_all_day: false,
      provider: "google",
      participants_json: JSON.stringify([
        {
          name: "Ada Lovelace",
          email: "ada@example.com",
          status: "accepted",
        },
      ]),
    });

    await notificationCommands.showNotification({
      key: `devtool-calendar-${eventId}`,
      title: "Devtool design sync",
      message: "Starting in 5 minutes",
      timeout: null,
      source: { type: "calendar_event", event_id: eventId },
      start_time: Math.floor(startedAt.getTime() / 1000),
      participants: [
        {
          name: "Ada Lovelace",
          email: "ada@example.com",
          status: "Accepted",
        },
      ],
      event_details: {
        what: "Devtool design sync",
        timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
        location: "Conference Room",
      },
      action_label: "Open Anarlog",
      action_variant: null,
      options: null,
      footer: null,
      icon: null,
    });
  }, [store, user_id]);

  const showMicDetectedNotification = useCallback(async () => {
    await notificationCommands.showNotification({
      key: `devtool-mic-${crypto.randomUUID()}`,
      title: "Are you in a meeting?",
      message: "",
      timeout: { secs: 15, nanos: 0 },
      source: {
        type: "mic_detected",
        app_names: ["Zoom"],
        app_ids: ["us.zoom.xos"],
        event_ids: [],
      },
      start_time: null,
      participants: null,
      event_details: null,
      action_label: null,
      action_variant: null,
      options: null,
      footer: null,
      icon: null,
    });
  }, []);

  const showMicOptionsNotification = useCallback(async () => {
    await notificationCommands.showNotification({
      key: `devtool-mic-options-${crypto.randomUUID()}`,
      title: "Are you in a meeting?",
      message: "",
      timeout: { secs: 15, nanos: 0 },
      source: {
        type: "mic_detected",
        app_names: ["Zoom", "Google Chrome"],
        app_ids: ["us.zoom.xos", "com.google.Chrome"],
        event_ids: [],
      },
      start_time: null,
      participants: null,
      event_details: null,
      action_label: null,
      action_variant: null,
      options: ["Design sync", "Customer call"],
      footer: {
        text: "Ignore Zoom and Chrome?",
        actionLabel: "Yes",
        icon: { type: "bundle_id", bundle_id: "us.zoom.xos" },
      },
      icon: null,
    });
  }, []);

  const showAutoStopNotification = useCallback(async () => {
    const sessionId =
      listenerStore.getState().live.sessionId ??
      `devtool-${crypto.randomUUID()}`;

    await notificationCommands.showNotification({
      key: createAutoStopEndedNotificationKey(sessionId),
      title: "Did your meeting end?",
      message: `Anarlog will stop listening in ${AUTO_STOP_CONFIRM_TIMEOUT_SECONDS} seconds.`,
      timeout: { secs: AUTO_STOP_CONFIRM_TIMEOUT_SECONDS, nanos: 0 },
      source: null,
      start_time: null,
      participants: null,
      event_details: null,
      action_label: "Stop",
      action_variant: "destructive",
      options: null,
      footer: null,
      icon: { type: "bundle_id", bundle_id: "com.google.Chrome" },
    });
  }, []);

  const createWithCountdown = useCallback(
    (seconds: number, meetingLink?: string) => {
      if (!store) {
        return;
      }

      const sessionId = crypto.randomUUID();
      const started_at = new Date(Date.now() + seconds * 1000).toISOString();
      const event_json = JSON.stringify({
        tracking_id: "devtool-test",
        calendar_id: "devtool-test",
        title: "Test Meeting",
        started_at,
        ended_at: new Date(
          Date.now() + seconds * 1000 + 30 * 60 * 1000,
        ).toISOString(),
        is_all_day: false,
        has_recurrence_rules: false,
        ...(meetingLink ? { meeting_link: meetingLink } : {}),
      });

      store.setRow("sessions", sessionId, {
        user_id: user_id ?? "",
        created_at: new Date().toISOString(),
        title: meetingLink ? "Countdown Test (Zoom)" : "Countdown Test",
        event_json,
      });

      openNew({ type: "sessions", id: sessionId });
    },
    [openNew, store, user_id],
  );

  const handleAction = useCallback(
    (action: string) => {
      switch (action as DevtoolsPanelAction) {
        case "navigation:onboarding":
          void showOnboarding();
          return;
        case "navigation:empty":
          void showEmptyTab();
          return;
        case "navigation:changelog":
          showChangelog();
          return;
        case "instruction:sign-in":
          showInstruction("sign-in");
          return;
        case "instruction:billing":
          showInstruction("billing");
          return;
        case "instruction:integration":
          showInstruction("integration");
          return;
        case "toasts:preview:language-model":
          void showToastPreviewInMainWindow("language-model");
          return;
        case "toasts:preview:transcription-model":
          void showToastPreviewInMainWindow("transcription-model");
          return;
        case "toasts:preview:transcription-error":
          void showToastPreviewInMainWindow("transcription-error");
          return;
        case "toasts:preview:download":
          void showToastPreviewInMainWindow("download");
          return;
        case "toasts:preview:pro":
          void showToastPreviewInMainWindow("pro");
          return;
        case "toasts:preview:clear":
          clearToastPreview();
          return;
        case "toasts:reset-dismissed":
          void commands.setDismissedToasts([]);
          return;
        case "ota:available":
          void showOtaPreviewInMainWindow("available");
          return;
        case "ota:downloading":
          void showOtaPreviewInMainWindow("downloading");
          return;
        case "ota:ready":
          void showOtaPreviewInMainWindow("ready");
          return;
        case "ota:failed":
          void showOtaPreviewInMainWindow("failed");
          return;
        case "ota:clear":
          clearOtaPreview();
          return;
        case "notifications:calendar":
          void showCalendarNotification();
          return;
        case "notifications:mic-detected":
          void showMicDetectedNotification();
          return;
        case "notifications:mic-options":
          void showMicOptionsNotification();
          return;
        case "notifications:auto-stop":
          void showAutoStopNotification();
          return;
        case "notifications:batch-done":
          void showBatchCompletedNotification("devtool", { force: true });
          return;
        case "notifications:clear":
          void notificationCommands.clearNotifications();
          return;
        case "billing:trial-started":
          setTrialStartedOpen(true);
          return;
        case "billing:trial-ended":
          setTrialEndedOpen(true);
          return;
        case "notes:populate-recurring":
          void populateRecurringNotes();
          return;
        case "countdown:note-20":
          createWithCountdown(20);
          return;
        case "countdown:note-60":
          createWithCountdown(60);
          return;
        case "countdown:note-290":
          createWithCountdown(290);
          return;
        case "countdown:zoom-20":
          createWithCountdown(20, "https://zoom.us/j/1234567890");
          return;
        case "countdown:zoom-60":
          createWithCountdown(60, "https://zoom.us/j/1234567890");
          return;
        case "error:trigger":
          setShouldThrow(true);
          return;
        default:
          console.warn("Unknown Devtools panel action:", action);
      }
    },
    [
      createWithCountdown,
      showAutoStopNotification,
      showCalendarNotification,
      showChangelog,
      showEmptyTab,
      showInstruction,
      showMicDetectedNotification,
      showMicOptionsNotification,
      showOnboarding,
      populateRecurringNotes,
      showToastPreviewInMainWindow,
      showOtaPreviewInMainWindow,
      clearToastPreview,
      clearOtaPreview,
    ],
  );

  return {
    dialogs: (
      <>
        <TrialStartedDialog
          open={trialStartedOpen}
          onOpenChange={setTrialStartedOpen}
          trialDaysRemaining={trialDaysRemaining}
        />
        <TrialEndedDialog
          open={trialEndedOpen}
          onOpenChange={setTrialEndedOpen}
          onUpgrade={upgradeToPro}
        />
      </>
    ),
    handleAction,
    shouldThrow,
  };
}

async function showDevtoolsPanel() {
  const result = await windowsCommands.devtoolsPanelShow();
  if (result.status === "error") {
    console.error("Failed to show Devtools panel:", result.error);
  }
}

async function hideDevtoolsPanel() {
  const result = await windowsCommands.devtoolsPanelHide();
  if (result.status === "error") {
    console.error("Failed to hide Devtools panel:", result.error);
  }
}
