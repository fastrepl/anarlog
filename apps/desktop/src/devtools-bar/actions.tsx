import { useCallback, useState } from "react";

import { commands as notificationCommands } from "@anlg/plugin-notification";
import { openUrlWithInstruction } from "@anlg/plugin-windows";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { populateRecurringMeetingNotes } from "./recurring-notes";

import { useBillingAccess } from "~/auth/billing-context";
import { TrialEndedDialog } from "~/billing/trial-ended-dialog";
import { TrialStartedDialog } from "~/billing/trial-started-dialog";
import { executeTransaction } from "~/db";
import { createSession, updateSession } from "~/session/queries";
import { useOwnerUserId } from "~/shared/owner-user";
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

export type DevtoolsAction =
  | "navigation:onboarding"
  | "instruction:sign-in"
  | "instruction:billing"
  | "instruction:integration"
  | `toasts:preview:${DevtoolsToastPreview}`
  | "toasts:clear"
  | `ota:${DevtoolsOtaPreviewStatus}`
  | "ota:clear"
  | "notifications:calendar"
  | "notifications:mic-detected"
  | "notifications:mic-options"
  | "notifications:auto-stop"
  | "notifications:batch-done"
  | "notifications:clear"
  | "billing:trial-started"
  | "billing:trial-ended"
  | "countdown:note-60"
  | "countdown:note-300"
  | "countdown:zoom-60"
  | "countdown:zoom-300"
  | "data:recurring-notes"
  | "error:trigger";

export type DevtoolsMenuItem = {
  label: string;
  action: DevtoolsAction;
  destructive?: boolean;
};

export const DEVTOOLS_MENU: Array<{
  label: string;
  items: DevtoolsMenuItem[];
}> = [
  {
    label: "Navigation",
    items: [
      { label: "Onboarding", action: "navigation:onboarding" },
      { label: "Instruction: sign-in", action: "instruction:sign-in" },
      { label: "Instruction: billing", action: "instruction:billing" },
      { label: "Instruction: integration", action: "instruction:integration" },
    ],
  },
  {
    label: "Toasts",
    items: [
      { label: "Language model", action: "toasts:preview:language-model" },
      {
        label: "Transcription model",
        action: "toasts:preview:transcription-model",
      },
      {
        label: "Transcription error",
        action: "toasts:preview:transcription-error",
      },
      { label: "Download", action: "toasts:preview:download" },
      { label: "Pro", action: "toasts:preview:pro" },
      { label: "Clear all toasts", action: "toasts:clear", destructive: true },
    ],
  },
  {
    label: "OTA",
    items: [
      { label: "Available", action: "ota:available" },
      { label: "Downloading", action: "ota:downloading" },
      { label: "Ready", action: "ota:ready" },
      { label: "Failed", action: "ota:failed" },
      { label: "Clear", action: "ota:clear", destructive: true },
    ],
  },
  {
    label: "Notifications",
    items: [
      { label: "Calendar", action: "notifications:calendar" },
      { label: "Mic detected", action: "notifications:mic-detected" },
      { label: "Mic options", action: "notifications:mic-options" },
      { label: "Auto-stop", action: "notifications:auto-stop" },
      { label: "Batch done", action: "notifications:batch-done" },
      { label: "Clear", action: "notifications:clear", destructive: true },
    ],
  },
  {
    label: "Billing",
    items: [
      { label: "Trial started", action: "billing:trial-started" },
      { label: "Trial ended", action: "billing:trial-ended" },
    ],
  },
  {
    label: "Countdown",
    items: [
      { label: "Note 1m", action: "countdown:note-60" },
      { label: "Note 5m", action: "countdown:note-300" },
      { label: "Zoom 1m", action: "countdown:zoom-60" },
      { label: "Zoom 5m", action: "countdown:zoom-300" },
    ],
  },
  {
    label: "Data",
    items: [
      { label: "Seed recurring meeting notes", action: "data:recurring-notes" },
    ],
  },
  {
    label: "Error",
    items: [
      { label: "Trigger error", action: "error:trigger", destructive: true },
    ],
  },
];

export function useDevtoolsActions() {
  const openNew = useTabs((state) => state.openNew);
  const userId = useOwnerUserId() ?? undefined;
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

  if (shouldThrow) {
    throw new Error("Test error triggered from devtools");
  }

  const showInstruction = useCallback((type: string) => {
    void openUrlWithInstruction(
      `https://example.com/${type}`,
      type,
      async () => ({ status: "ok" as const }),
    );
  }, []);

  const clearNotifications = useCallback(async () => {
    try {
      await notificationCommands.clearNotifications();
    } catch (error) {
      console.error("[devtools] failed to clear notifications", error);
    }
  }, []);

  const showCalendarNotification = useCallback(async () => {
    const eventId = `devtool-event-${crypto.randomUUID()}`;
    const startedAt = new Date(Date.now() + 5 * 60 * 1000);
    const endedAt = new Date(startedAt.getTime() + 30 * 60 * 1000);
    const now = new Date().toISOString();

    await executeTransaction([
      {
        sql: `
          INSERT INTO events (
            id, tracking_id_event, calendar_id, title, started_at, ended_at,
            location, meeting_link, description, note, recurrence_series_id,
            has_recurrence_rules, is_all_day, provider, participants_json,
            created_at, updated_at, deleted_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, '', '', 0, 0, ?, ?, ?, ?, NULL)
        `,
        params: [
          eventId,
          eventId,
          "devtool-calendar",
          "Devtool design sync",
          startedAt.toISOString(),
          endedAt.toISOString(),
          "Conference Room",
          "https://zoom.us/j/1234567890",
          "Notification test event",
          "google",
          JSON.stringify([
            {
              name: "Ada Lovelace",
              email: "ada@example.com",
              status: "accepted",
            },
          ]),
          now,
          now,
        ],
      },
    ]);

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
  }, []);

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
      title: "Are you in Design sync right now?",
      message: "",
      timeout: { secs: 15, nanos: 0 },
      source: {
        type: "mic_detected",
        app_names: ["Zoom", "Google Chrome"],
        app_ids: ["us.zoom.xos", "com.google.Chrome"],
        event_ids: ["devtool-event-1"],
      },
      start_time: null,
      participants: null,
      event_details: null,
      action_label: "Yes",
      action_variant: null,
      options: null,
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
    async (seconds: number, meetingLink?: string) => {
      if (!userId) {
        return;
      }

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

      const sessionId = await createSession(
        meetingLink ? "Countdown Test (Zoom)" : "Countdown Test",
        userId,
      );
      await updateSession(sessionId, {
        created_at: new Date().toISOString(),
        event_json,
      });

      openNew({ type: "sessions", id: sessionId });
    },
    [openNew, userId],
  );

  const seedRecurringNotes = useCallback(async () => {
    try {
      const sessionId = await populateRecurringMeetingNotes({ userId });
      openNew({ type: "sessions", id: sessionId });
    } catch (error) {
      sonnerToast.error(
        error instanceof Error ? error.message : "Failed to seed notes",
      );
    }
  }, [openNew, userId]);

  const run = useCallback(
    (action: DevtoolsAction) => {
      switch (action) {
        case "navigation:onboarding":
          openNew({ type: "onboarding" });
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
        case "toasts:preview:transcription-model":
        case "toasts:preview:transcription-error":
        case "toasts:preview:download":
        case "toasts:preview:pro":
          showToastPreview(
            action.slice("toasts:preview:".length) as DevtoolsToastPreview,
          );
          return;
        case "toasts:clear":
          clearToastPreview();
          return;
        case "ota:available":
        case "ota:downloading":
        case "ota:ready":
        case "ota:failed":
          showOtaPreview(
            action.slice("ota:".length) as DevtoolsOtaPreviewStatus,
          );
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
          void clearNotifications();
          return;
        case "billing:trial-started":
          setTrialStartedOpen(true);
          return;
        case "billing:trial-ended":
          setTrialEndedOpen(true);
          return;
        case "countdown:note-60":
          void createWithCountdown(60);
          return;
        case "countdown:note-300":
          void createWithCountdown(300);
          return;
        case "countdown:zoom-60":
          void createWithCountdown(60, "https://zoom.us/j/1234567890");
          return;
        case "countdown:zoom-300":
          void createWithCountdown(300, "https://zoom.us/j/1234567890");
          return;
        case "data:recurring-notes":
          void seedRecurringNotes();
          return;
        case "error:trigger":
          setShouldThrow(true);
          return;
      }
    },
    [
      clearNotifications,
      clearOtaPreview,
      clearToastPreview,
      createWithCountdown,
      openNew,
      seedRecurringNotes,
      showAutoStopNotification,
      showCalendarNotification,
      showInstruction,
      showMicDetectedNotification,
      showMicOptionsNotification,
      showOtaPreview,
      showToastPreview,
    ],
  );

  return {
    run,
    dialogs: (
      <>
        <TrialStartedDialog
          open={trialStartedOpen}
          onOpenChange={setTrialStartedOpen}
          trialDaysRemaining={trialDaysRemaining}
          hasPaymentMethod={false}
        />
        <TrialEndedDialog
          open={trialEndedOpen}
          onOpenChange={setTrialEndedOpen}
          onUpgrade={upgradeToPro}
        />
      </>
    ),
  };
}
