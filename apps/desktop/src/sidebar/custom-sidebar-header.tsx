import { ArrowLeftIcon } from "lucide-react";
import { useCallback } from "react";

import { cn } from "@hypr/utils";

import { useShell } from "~/contexts/shell";
import { useTabs } from "~/store/zustand/tabs";

export function CustomSidebarHeader({
  title,
  children,
}: {
  title: string;
  children?: React.ReactNode;
}) {
  const { chat } = useShell();
  const currentTab = useTabs((state) => state.currentTab);
  const tabs = useTabs((state) => state.tabs);
  const select = useTabs((state) => state.select);
  const openCurrent = useTabs((state) => state.openCurrent);

  const handleBack = useCallback(() => {
    if (chat.mode === "FloatingOpen") {
      chat.sendEvent({ type: "CLOSE" });
      return;
    }

    if (currentTab?.type === "onboarding" || currentTab?.type === "empty") {
      return;
    }

    const existingHomeTab = tabs.find((tab) => tab.type === "empty");
    if (existingHomeTab) {
      select(existingHomeTab);
      return;
    }

    openCurrent({ type: "empty" });
  }, [chat, currentTab, openCurrent, select, tabs]);

  return (
    <div
      data-tauri-drag-region
      className="-mt-11 flex h-12 shrink-0 items-start py-0 pt-[9px] pr-1 pl-[76px]"
    >
      <div
        data-tauri-drag-region
        className="flex min-w-0 flex-1 items-center gap-1"
      >
        <button
          type="button"
          aria-label="Go home"
          title="Back"
          data-tauri-drag-region="false"
          className={cn([
            "relative z-50 flex size-6 shrink-0 items-center justify-center rounded-full",
            "text-neutral-600 transition-colors hover:bg-neutral-200/60 hover:text-neutral-900",
            "focus-visible:ring-2 focus-visible:ring-neutral-900 focus-visible:outline-hidden",
          ])}
          onClick={handleBack}
        >
          <ArrowLeftIcon size={14} />
        </button>
        <h3 className="truncate font-sans text-sm font-medium select-none">
          {title}
        </h3>
      </div>
      {children ? (
        <div
          data-tauri-drag-region="false"
          className="ml-1 flex shrink-0 items-center"
        >
          {children}
        </div>
      ) : null}
    </div>
  );
}
