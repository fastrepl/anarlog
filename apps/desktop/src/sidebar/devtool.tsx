import { useCallback, useState } from "react";

import { commands as windowsCommands } from "@hypr/plugin-windows";
import { cn } from "@hypr/utils";

import { getLatestVersion } from "./changelog";

import { useTabs } from "~/store/zustand/tabs";

export function DevtoolView() {
  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex flex-1 flex-col gap-2 overflow-y-auto px-1 py-2">
        <NavigationCard />
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

  const handleShowControl = useCallback(() => {
    void windowsCommands.windowShow({ type: "control" });
  }, []);

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
        <button
          type="button"
          onClick={handleShowControl}
          className={cn([
            "w-full rounded-md px-2.5 py-1.5",
            "text-left text-xs font-medium",
            "border border-neutral-200 text-neutral-700",
            "cursor-pointer transition-colors",
            "hover:border-neutral-300 hover:bg-neutral-50",
          ])}
        >
          Control
        </button>
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
