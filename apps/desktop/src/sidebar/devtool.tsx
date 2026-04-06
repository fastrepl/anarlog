import { useCallback, useState } from "react";

import {
  commands as windowsCommands,
  openUrlWithInstruction,
} from "@hypr/plugin-windows";
import { cn } from "@hypr/utils";

import { getLatestVersion } from "~/changelog";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";
import { commands } from "~/types/tauri.gen";

export function DevtoolView() {
  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex flex-1 flex-col gap-2 overflow-y-auto px-1 py-2">
        <NavigationCard />
        <ToastsCard />
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
}: {
  title: string;
  children: React.ReactNode;
  maxHeight?: string;
}) {
  return (
    <div
      className={cn([
        "rounded-lg border border-neutral-200 bg-white",
        "shadow-xs",
        "overflow-hidden",
        "shrink-0",
      ])}
    >
      <div className="border-b border-neutral-100 bg-neutral-50 px-2 py-1.5">
        <h2 className="text-xs font-semibold tracking-wide text-neutral-600 uppercase">
          {title}
        </h2>
      </div>
      <div
        className="overflow-y-auto p-2"
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
  const handleResetDismissed = useCallback(async () => {
    await commands.setDismissedToasts([]);
  }, []);

  const handleResetShareOnly = useCallback(async () => {
    const result = await commands.getDismissedToasts();
    if (result.status === "ok") {
      await commands.setDismissedToasts(
        result.data.filter(
          (id: string) =>
            id !== "share-char" && !id.startsWith("share-char-snoozed:"),
        ),
      );
    }
  }, []);

  const btnClass = cn([
    "w-full rounded-md px-2.5 py-1.5",
    "text-left text-xs font-medium",
    "border border-neutral-200 text-neutral-700",
    "cursor-pointer transition-colors",
    "hover:border-neutral-300 hover:bg-neutral-50",
  ]);

  return (
    <DevtoolCard title="Toasts">
      <div className="flex flex-col gap-1.5">
        <button
          type="button"
          onClick={() => void handleResetShareOnly()}
          className={btnClass}
        >
          Reset Share Toast
        </button>
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
