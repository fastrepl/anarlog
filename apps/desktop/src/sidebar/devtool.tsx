import { useCallback, useState } from "react";

import { commands as notificationCommands } from "@hypr/plugin-notification";
import {
  commands as windowsCommands,
  openUrlWithInstruction,
} from "@hypr/plugin-windows";
import { cn } from "@hypr/utils";

import { useBillingAccess } from "~/auth/billing";
import { TrialEndedDialog } from "~/billing/trial-ended-dialog";
import { TrialStartedDialog } from "~/billing/trial-started-dialog";
import { getLatestVersion } from "~/changelog";
import { Toast } from "~/sidebar/toast/component";
import type { ToastType } from "~/sidebar/toast/types";
import * as main from "~/store/tinybase/store/main";
import { showBatchCompletedNotification } from "~/store/zustand/listener/general-batch";
import { listenerStore } from "~/store/zustand/listener/instance";
import { useTabs } from "~/store/zustand/tabs";
import { createAutoStopEndedNotificationKey } from "~/stt/auto-stop-notification";
import { commands } from "~/types/tauri.gen";

export function DevtoolView() {
  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex flex-1 flex-col gap-2 overflow-y-auto px-1 py-2">
        <NavigationCard />
        <ToastsCard />
        <NotificationsCard />
        <BillingCard />
        <CountdownTestCard />
        <ErrorTestCard />
      </div>
    </div>
  );
}

function DevtoolCard({
  title,
  children,
  maxHeight,
  overflowVisible = false,
}: {
  title: string;
  children: React.ReactNode;
  maxHeight?: string;
  overflowVisible?: boolean;
}) {
  return (
    <div
      className={cn([
        "rounded-lg border border-neutral-200 bg-white",
        "shadow-xs",
        overflowVisible ? "overflow-visible" : "overflow-hidden",
        "shrink-0",
      ])}
    >
      <div className="border-b border-neutral-100 bg-neutral-50 px-2 py-1.5">
        <h2 className="text-xs font-semibold tracking-wide text-neutral-600 uppercase">
          {title}
        </h2>
      </div>
      <div
        className={cn([
          maxHeight ? "overflow-y-auto" : "overflow-visible",
          "p-2",
        ])}
        style={maxHeight ? { maxHeight } : undefined}
      >
        {children}
      </div>
    </div>
  );
}

function NavigationCard() {
  const openNew = useTabs((s) => s.openNew);
  const isClassicMain =
    typeof window !== "undefined" &&
    (window.location.pathname === "/app/main" ||
      window.location.pathname.startsWith("/app/main/"));

  const showMainWindow = useCallback(async () => {
    await windowsCommands.windowShow({ type: "main" });
  }, []);

  const handleShowEmptyTab = useCallback(async () => {
    await showMainWindow();
    openNew({ type: "empty" });
  }, [openNew, showMainWindow]);

  const handleShowOnboarding = useCallback(async () => {
    await showMainWindow();
    openNew({ type: "onboarding" });
  }, [openNew, showMainWindow]);

  const showInstruction = useCallback(
    (type: string) =>
      openUrlWithInstruction(`https://example.com/${type}`, type, async () => ({
        status: "ok" as const,
      })),
    [],
  );

  const handleShowChangelog = useCallback(() => {
    const latestVersion = getLatestVersion();
    if (latestVersion) {
      openNew({
        type: "changelog",
        state: { current: latestVersion, previous: null },
      });
    }
  }, [openNew]);

  return (
    <DevtoolCard title="Navigation">
      <div className="flex flex-col gap-1.5">
        <button
          type="button"
          onClick={() => void handleShowOnboarding()}
          className={cn([
            "w-full rounded-md px-2.5 py-1.5",
            "text-left text-xs font-medium",
            "border border-neutral-200 text-neutral-700",
            "cursor-pointer transition-colors",
            "hover:border-neutral-300 hover:bg-neutral-50",
          ])}
        >
          Onboarding Tab
        </button>
        {isClassicMain && (
          <button
            type="button"
            onClick={() => void handleShowEmptyTab()}
            className={cn([
              "w-full rounded-md px-2.5 py-1.5",
              "text-left text-xs font-medium",
              "border border-neutral-200 text-neutral-700",
              "cursor-pointer transition-colors",
              "hover:border-neutral-300 hover:bg-neutral-50",
            ])}
          >
            Empty Tab
          </button>
        )}
        {["sign-in", "billing", "integration"].map((type) => (
          <button
            key={type}
            type="button"
            onClick={() => void showInstruction(type)}
            className={cn([
              "w-full rounded-md px-2.5 py-1.5",
              "text-left text-xs font-medium",
              "border border-neutral-200 text-neutral-700",
              "cursor-pointer transition-colors",
              "hover:border-neutral-300 hover:bg-neutral-50",
            ])}
          >
            Instruction: {type}
          </button>
        ))}
        <button
          type="button"
          onClick={handleShowChangelog}
          className={cn([
            "w-full rounded-md px-2.5 py-1.5",
            "text-left text-xs font-medium",
            "border border-neutral-200 text-neutral-700",
            "cursor-pointer transition-colors",
            "hover:border-neutral-300 hover:bg-neutral-50",
          ])}
        >
          Changelog
        </button>
      </div>
    </DevtoolCard>
  );
}

function ToastsCard() {
  const [previewId, setPreviewId] = useState<ToastPreviewId>("missing-llm");

  const handleResetDismissed = useCallback(async () => {
    await commands.setDismissedToasts([]);
  }, []);

  const preview =
    TOAST_PREVIEWS.find((item) => item.id === previewId) ?? TOAST_PREVIEWS[0];

  const btnClass = cn([
    "w-full rounded-md px-2.5 py-1.5",
    "text-left text-xs font-medium",
    "border border-neutral-200 text-neutral-700",
    "cursor-pointer transition-colors",
    "hover:border-neutral-300 hover:bg-neutral-50",
  ]);

  return (
    <DevtoolCard title="Toasts" overflowVisible>
      <div className="flex flex-col gap-1.5">
        <label className="flex flex-col gap-1">
          <span className="text-xs font-medium text-neutral-600">
            Preview toast
          </span>
          <select
            value={preview.id}
            onChange={(event) =>
              setPreviewId(event.currentTarget.value as ToastPreviewId)
            }
            className={cn([
              "w-full rounded-md px-2.5 py-1.5",
              "border border-neutral-200 bg-white",
              "text-xs font-medium text-neutral-700",
              "cursor-pointer transition-colors",
              "hover:border-neutral-300 hover:bg-neutral-50",
            ])}
          >
            {TOAST_PREVIEWS.map((item) => (
              <option key={item.id} value={item.id}>
                {item.label}
              </option>
            ))}
          </select>
        </label>
        <div className="-mx-1 overflow-visible">
          <Toast
            toast={preview.toast}
            onDismiss={preview.toast.dismissible ? noop : undefined}
          />
        </div>
        <button
          type="button"
          onClick={() => void handleResetDismissed()}
          className={btnClass}
        >
          Reset All Dismissed
        </button>
      </div>
    </DevtoolCard>
  );
}

type ToastPreviewId =
  | "missing-llm"
  | "missing-stt"
  | "stt-unavailable"
  | "download"
  | "pro";

const noop = () => {};

const TOAST_PREVIEWS: {
  id: ToastPreviewId;
  label: string;
  toast: ToastType;
}[] = [
  {
    id: "missing-llm",
    label: "Language model",
    toast: {
      id: "missing-llm-preview",
      description: "Language model needed",
      dismissible: true,
      primaryAction: { label: "Add", onClick: noop },
    },
  },
  {
    id: "missing-stt",
    label: "Transcription model",
    toast: {
      id: "missing-stt-preview",
      description: "Transcription model needed",
      dismissible: false,
      primaryAction: { label: "Add", onClick: noop },
    },
  },
  {
    id: "stt-unavailable",
    label: "Transcription error",
    toast: {
      id: "stt-unavailable-preview",
      description: "Transcription unavailable",
      dismissible: true,
      variant: "error",
      primaryAction: { label: "Settings", onClick: noop },
    },
  },
  {
    id: "download",
    label: "Download",
    toast: {
      id: "download-preview",
      description: "Downloading model",
      dismissible: false,
      progress: 42,
    },
  },
  {
    id: "pro",
    label: "Pro",
    toast: {
      id: "pro-preview",
      description: "Pro features available",
      dismissible: true,
      primaryAction: { label: "Upgrade", onClick: noop },
    },
  },
];

function NotificationsCard() {
  const store = main.UI.useStore(main.STORE_ID) as main.Store | undefined;
  const { user_id } = main.UI.useValues(main.STORE_ID);

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
      action_label: "Open notes",
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
      options: null,
      footer: null,
      icon: null,
    });
  }, []);

  const showMicDetectedOptionsNotification = useCallback(async () => {
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
      options: ["Design sync", "Customer call"],
      footer: {
        text: "Ignore Zoom and Chrome?",
        actionLabel: "Yes",
        icon: { type: "bundle_id", bundle_id: "us.zoom.xos" },
      },
      icon: null,
    });
  }, []);

  const showAutoStopConfirmationNotification = useCallback(async () => {
    const sessionId =
      listenerStore.getState().live.sessionId ??
      `devtool-${crypto.randomUUID()}`;

    await notificationCommands.showNotification({
      key: createAutoStopEndedNotificationKey(sessionId),
      title: "Did your meeting end?",
      message:
        "Google Chrome stopped using the microphone before the scheduled end time.",
      timeout: { secs: 60, nanos: 0 },
      source: null,
      start_time: null,
      participants: null,
      event_details: null,
      action_label: "Stop recording",
      options: null,
      footer: null,
      icon: { type: "bundle_id", bundle_id: "com.google.Chrome" },
    });
  }, []);

  const showBatchNotification = useCallback(async () => {
    await showBatchCompletedNotification("devtool", { force: true });
  }, []);

  const clearNotifications = useCallback(async () => {
    await notificationCommands.clearNotifications();
  }, []);

  const btnClass = cn([
    "w-full rounded-md px-2.5 py-1.5",
    "text-left text-xs font-medium",
    "border border-neutral-200 text-neutral-700",
    "cursor-pointer transition-colors",
    "hover:border-neutral-300 hover:bg-neutral-50",
  ]);

  return (
    <DevtoolCard title="Notifications">
      <div className="flex flex-col gap-1.5">
        <button
          type="button"
          onClick={() => void showCalendarNotification()}
          className={btnClass}
        >
          Calendar event
        </button>
        <button
          type="button"
          onClick={() => void showMicDetectedNotification()}
          className={btnClass}
        >
          Mic detected
        </button>
        <button
          type="button"
          onClick={() => void showMicDetectedOptionsNotification()}
          className={btnClass}
        >
          Mic detected with options
        </button>
        <button
          type="button"
          onClick={() => void showAutoStopConfirmationNotification()}
          className={btnClass}
        >
          Auto-stop confirmation
        </button>
        <button
          type="button"
          onClick={() => void showBatchNotification()}
          className={btnClass}
        >
          Batch transcription complete
        </button>
        <button
          type="button"
          onClick={() => void clearNotifications()}
          className={btnClass}
        >
          Clear notifications
        </button>
      </div>
    </DevtoolCard>
  );
}

function BillingCard() {
  const { trialDaysRemaining, upgradeToPro } = useBillingAccess();
  const [trialStartedOpen, setTrialStartedOpen] = useState(false);
  const [trialEndedOpen, setTrialEndedOpen] = useState(false);

  const btnClass = cn([
    "w-full rounded-md px-2.5 py-1.5",
    "text-left text-xs font-medium",
    "border border-neutral-200 text-neutral-700",
    "cursor-pointer transition-colors",
    "hover:border-neutral-300 hover:bg-neutral-50",
  ]);

  return (
    <DevtoolCard title="Billing">
      <div className="flex flex-col gap-1.5">
        <button
          type="button"
          onClick={() => setTrialStartedOpen(true)}
          className={btnClass}
        >
          Pro trial started modal
        </button>
        <button
          type="button"
          onClick={() => setTrialEndedOpen(true)}
          className={btnClass}
        >
          Pro trial ended modal
        </button>
      </div>
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
    </DevtoolCard>
  );
}

function CountdownTestCard() {
  const store = main.UI.useStore(main.STORE_ID) as main.Store | undefined;
  const { user_id } = main.UI.useValues(main.STORE_ID);
  const openNew = useTabs((s) => s.openNew);

  const createWithCountdown = useCallback(
    (seconds: number, meetingLink?: string) => {
      if (!store) return;
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
        ...(meetingLink && { meeting_link: meetingLink }),
      });

      store.setRow("sessions", sessionId, {
        user_id: user_id ?? "",
        created_at: new Date().toISOString(),
        title: meetingLink ? "Countdown Test (Zoom)" : "Countdown Test",
        event_json,
      });

      openNew({ type: "sessions", id: sessionId });
    },
    [store, user_id, openNew],
  );

  const btnClass = cn([
    "w-full rounded-md px-2.5 py-1.5",
    "text-left text-xs font-medium",
    "border border-neutral-200 text-neutral-700",
    "cursor-pointer transition-colors",
    "hover:border-neutral-300 hover:bg-neutral-50",
    "disabled:cursor-not-allowed disabled:opacity-40",
  ]);

  return (
    <DevtoolCard title="Countdown">
      <div className="flex flex-col gap-1.5">
        <button
          type="button"
          disabled={!store}
          onClick={() => createWithCountdown(20)}
          className={btnClass}
        >
          +Note starting in 20s
        </button>
        <button
          type="button"
          disabled={!store}
          onClick={() => createWithCountdown(60)}
          className={btnClass}
        >
          +Note starting in 60s
        </button>
        <button
          type="button"
          disabled={!store}
          onClick={() => createWithCountdown(290)}
          className={btnClass}
        >
          +Note starting in ~5m
        </button>
        <hr className="border-neutral-100" />
        <button
          type="button"
          disabled={!store}
          onClick={() =>
            createWithCountdown(20, "https://zoom.us/j/1234567890")
          }
          className={btnClass}
        >
          +Zoom in 20s
        </button>
        <button
          type="button"
          disabled={!store}
          onClick={() =>
            createWithCountdown(60, "https://zoom.us/j/1234567890")
          }
          className={btnClass}
        >
          +Zoom in 60s
        </button>
      </div>
    </DevtoolCard>
  );
}

function ErrorTestCard() {
  const [shouldThrow, setShouldThrow] = useState(false);

  const handleTriggerError = useCallback(() => {
    setShouldThrow(true);
  }, []);

  if (shouldThrow) {
    throw new Error("Test error triggered from devtools");
  }

  return (
    <DevtoolCard title="Error Testing">
      <div className="flex flex-col gap-1.5">
        <button
          type="button"
          onClick={handleTriggerError}
          className={cn([
            "w-full rounded-md px-2.5 py-1.5",
            "text-left text-xs font-medium",
            "border border-red-200 bg-red-50 text-red-700",
            "cursor-pointer transition-colors",
            "hover:border-red-300 hover:bg-red-100",
          ])}
        >
          Trigger Error
        </button>
      </div>
    </DevtoolCard>
  );
}
