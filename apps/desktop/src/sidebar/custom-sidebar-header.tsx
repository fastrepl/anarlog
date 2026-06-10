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
    if (chat.mode !== "FloatingClosed") {
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
        <CustomSidebarHeaderButton
          label="Go home"
          title="Back"
          onClick={handleBack}
        >
          <ArrowLeftIcon size={14} />
        </CustomSidebarHeaderButton>
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

function CustomSidebarHeaderButton({
  children,
  disabled = false,
  label,
  onClick,
  title,
}: {
  children: React.ReactNode;
  disabled?: boolean;
  label: string;
  onClick: () => void;
  title?: string;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={title}
      data-tauri-drag-region="false"
      disabled={disabled}
      className={cn([
        "relative z-50 flex size-6 shrink-0 items-center justify-center rounded-full",
        "text-muted-foreground hover:bg-accent hover:text-foreground transition-colors",
        "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
        "disabled:text-muted-foreground/70 disabled:hover:text-muted-foreground/70 disabled:hover:bg-transparent",
      ])}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
