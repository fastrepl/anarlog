import { AlertCircleIcon, Pin, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { Kbd } from "@hypr/ui/components/ui/kbd";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { useCmdKeyPressed } from "@hypr/ui/hooks/use-cmd-key-pressed";
import { cn } from "@hypr/utils";

import { InteractiveButton } from "~/shared/ui/interactive-button";
import { type Tab } from "~/store/zustand/tabs";

type TabItemProps<T extends Tab = Tab> = { tab: T; tabIndex?: number } & {
  handleSelectThis: (tab: T) => void;
  handleCloseThis: (tab: T) => void;
  handleCloseOthers: () => void;
  handleCloseAll: () => void;
  handlePinThis: (tab: T) => void;
  handleUnpinThis: (tab: T) => void;
  pendingCloseConfirmationTab?: Tab | null;
  setPendingCloseConfirmationTab?: (tab: Tab | null) => void;
};

type TabAccent = "neutral" | "red" | "amber" | "blue";

const accentColors: Record<
  TabAccent,
  {
    selected: string[];
    unselected: string[];
    hover: { selected: string; unselected: string };
  }
> = {
  neutral: {
    selected: [
      "bg-neutral-200/50",
      "hover:bg-neutral-200",
      "text-black",
      "border-stone-400",
    ],
    unselected: [
      "bg-neutral-50",
      "hover:bg-stone-100",
      "text-neutral-500",
      "border-transparent",
    ],
    hover: {
      selected: "text-neutral-700 hover:text-neutral-900",
      unselected: "text-neutral-500 hover:text-neutral-700",
    },
  },
  red: {
    selected: ["bg-red-50", "text-red-600", "border-red-400"],
    unselected: ["bg-red-50", "text-red-500", "border-transparent"],
    hover: {
      selected: "text-red-600 hover:text-red-700",
      unselected: "text-red-600 hover:text-red-700",
    },
  },
  amber: {
    selected: ["bg-amber-50", "text-amber-600", "border-amber-400"],
    unselected: ["bg-amber-50", "text-amber-500", "border-transparent"],
    hover: {
      selected: "text-amber-600 hover:text-amber-700",
      unselected: "text-amber-600 hover:text-amber-700",
    },
  },
  blue: {
    selected: ["bg-sky-50", "text-sky-700", "border-sky-400"],
    unselected: ["bg-sky-50", "text-sky-500", "border-transparent"],
    hover: {
      selected: "text-sky-500 hover:text-sky-700",
      unselected: "text-sky-400 hover:text-sky-600",
    },
  },
};

import type { TabStatus } from "~/session/tab-visual-state";

function statusToAccent(status: TabStatus | undefined): TabAccent {
  switch (status) {
    case "listening":
      return "red";
    case "listening-degraded":
      return "amber";
    default:
      return "neutral";
  }
}

function statusRequiresConfirmation(status: TabStatus | undefined): boolean {
  return (
    status === "listening" ||
    status === "listening-degraded" ||
    status === "finalizing"
  );
}

function statusShowsSpinner(status: TabStatus | undefined): boolean {
  return status === "finalizing" || status === "processing";
}

type TabItemBaseProps = {
  icon: React.ReactNode;
  title: React.ReactNode;
  selected: boolean;
  status?: TabStatus;
  accent?: TabAccent;
  pinned?: boolean;
  allowPin?: boolean;
  allowClose?: boolean;
  isEmptyTab?: boolean;
  tabIndex?: number;
  showCloseConfirmation?: boolean;
  onCloseConfirmationChange?: (show: boolean) => void;
} & {
  handleCloseThis: () => void;
  handleSelectThis: () => void;
  handleCloseOthers: () => void;
  handleCloseAll: () => void;
  handlePinThis: () => void;
  handleUnpinThis: () => void;
};

export type TabItem<T extends Tab = Tab> = (
  props: TabItemProps<T>,
) => React.ReactNode;

export function TabItemBase({
  icon,
  title,
  selected,
  status,
  accent: accentOverride,
  pinned = false,
  allowPin = true,
  allowClose = true,
  isEmptyTab = false,
  tabIndex,
  showCloseConfirmation = false,
  onCloseConfirmationChange,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
}: TabItemBaseProps) {
  const accent = accentOverride ?? statusToAccent(status);
  const active = statusRequiresConfirmation(status);
  const showSpinner = statusShowsSpinner(status);

  const colors = accentColors[accent];
  const isCmdPressed = useCmdKeyPressed();
  const [isHovered, setIsHovered] = useState(false);
  const [localShowConfirmation, setLocalShowConfirmation] = useState(false);

  const isConfirmationOpen = showCloseConfirmation || localShowConfirmation;

  useEffect(() => {
    if (showCloseConfirmation) {
      setLocalShowConfirmation(true);
    }
  }, [showCloseConfirmation]);

  const handleCloseConfirmationChange = (open: boolean) => {
    setLocalShowConfirmation(open);
    onCloseConfirmationChange?.(open);
  };

  const handleAttemptClose = () => {
    if (active) {
      handleCloseConfirmationChange(true);
    } else {
      handleCloseThis();
    }
  };

  const handleConfirmClose = useCallback(() => {
    setLocalShowConfirmation(false);
    onCloseConfirmationChange?.(false);
    handleCloseThis();
  }, [handleCloseThis, onCloseConfirmationChange]);

  useEffect(() => {
    if (!isConfirmationOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "w") {
        e.preventDefault();
        e.stopPropagation();
        handleConfirmClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown, { capture: true });
    return () => {
      window.removeEventListener("keydown", handleKeyDown, { capture: true });
    };
  }, [isConfirmationOpen, handleConfirmClose]);

  const handleMouseDown = (e: React.MouseEvent) => {
    if (e.button === 1 && !active && allowClose) {
      e.preventDefault();
      e.stopPropagation();
      handleCloseThis();
    }
  };

  const contextMenu = allowClose
    ? active || (selected && !isEmptyTab)
      ? [
          { id: "close-tab", text: "Close", action: handleAttemptClose },
          {
            id: "close-others",
            text: "Close others",
            action: handleCloseOthers,
          },
          { id: "close-all", text: "Close all", action: handleCloseAll },
          ...(allowPin
            ? [
                { separator: true as const },
                pinned
                  ? {
                      id: "unpin-tab",
                      text: "Unpin tab",
                      action: handleUnpinThis,
                    }
                  : { id: "pin-tab", text: "Pin tab", action: handlePinThis },
              ]
            : []),
        ]
      : [
          { id: "close-tab", text: "Close", action: handleAttemptClose },
          {
            id: "close-others",
            text: "Close others",
            action: handleCloseOthers,
          },
          { id: "close-all", text: "Close all", action: handleCloseAll },
          ...(allowPin
            ? [
                { separator: true as const },
                pinned
                  ? {
                      id: "unpin-tab",
                      text: "Unpin tab",
                      action: handleUnpinThis,
                    }
                  : { id: "pin-tab", text: "Pin tab", action: handlePinThis },
              ]
            : []),
        ]
    : [];

  const showShortcut = isCmdPressed && tabIndex !== undefined;

  const indicatorDot =
    status === "listening" ? (
      <div className="relative size-2">
        <div className="absolute inset-0 rounded-full bg-red-600"></div>
        <div className="absolute inset-0 animate-ping rounded-full bg-red-300"></div>
      </div>
    ) : status === "listening-degraded" ? (
      <AlertCircleIcon className="size-4 text-amber-500" />
    ) : null;

  return (
    <div
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      className="relative h-8"
    >
      <InteractiveButton
        asChild
        contextMenu={contextMenu}
        onClick={handleSelectThis}
        onMouseDown={handleMouseDown}
        className={cn([
          "relative flex items-center gap-1",
          "h-8 w-[160px] px-2",
          "rounded-xl border",
          "group cursor-pointer",
          "transition-colors duration-200",
          selected ? colors.selected : colors.unselected,
        ])}
      >
        <div className="flex min-w-0 flex-1 items-center gap-2 text-sm">
          <div className="flex h-4 w-4 shrink-0 items-center justify-center">
            {showSpinner ? <Spinner size={16} /> : (indicatorDot ?? icon)}
          </div>
          <span className="pointer-events-none truncate">{title}</span>
        </div>
        <div
          className={cn([
            "relative shrink-0 overflow-visible",
            showShortcut
              ? "flex h-5 min-w-fit items-center justify-end"
              : "h-4 w-4",
          ])}
        >
          <div
            className={cn([
              "absolute inset-0 flex items-center justify-center transition-opacity duration-200",
              showShortcut || ((isHovered || isConfirmationOpen) && allowClose)
                ? "opacity-0"
                : "opacity-100",
            ])}
          >
            {pinned && (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  handleUnpinThis();
                }}
                className={cn([
                  "flex items-center justify-center transition-colors",
                  colors.hover[selected ? "selected" : "unselected"],
                ])}
              >
                <Pin size={14} />
              </button>
            )}
          </div>
          {allowClose && (
            <div
              className={cn([
                "absolute inset-0 flex items-center justify-center transition-opacity duration-200",
                showShortcut || !(isHovered || isConfirmationOpen)
                  ? "opacity-0"
                  : "opacity-100",
              ])}
            >
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  handleAttemptClose();
                }}
                className={cn([
                  "flex items-center justify-center transition-colors",
                  colors.hover[selected ? "selected" : "unselected"],
                ])}
              >
                <X size={16} />
              </button>
            </div>
          )}
          {showShortcut && (
            <div className="pointer-events-none flex h-full items-center justify-end">
              <Kbd>⌘ {tabIndex}</Kbd>
            </div>
          )}
        </div>
      </InteractiveButton>
      <Popover
        open={active && isConfirmationOpen}
        onOpenChange={handleCloseConfirmationChange}
      >
        <PopoverTrigger asChild>
          <div className="pointer-events-none absolute inset-0" />
        </PopoverTrigger>
        <PopoverContent
          variant="app"
          side="bottom"
          align="start"
          className="z-[60] w-[240px]"
          sideOffset={2}
          onOpenAutoFocus={(e) => e.preventDefault()}
        >
          <AppFloatingPanel className="flex flex-col gap-2 p-3">
            <p className="text-sm text-neutral-700">
              Are you sure you want to close this tab? This will stop Char from
              listening.
            </p>
            <Button
              variant="destructive"
              className="group relative flex h-9 w-full items-center justify-center rounded-lg"
              onClick={(e) => {
                e.stopPropagation();
                handleConfirmClose();
              }}
            >
              <span>Close</span>
              <Kbd
                className={cn([
                  "absolute right-2",
                  "border-red-200/30 bg-red-200/20 text-red-100",
                  "transition-all duration-100",
                  "group-hover:-translate-y-0.5 group-hover:shadow-[0_2px_0_0_rgba(0,0,0,0.15),inset_0_1px_0_0_rgba(255,255,255,0.8)]",
                  "group-active:translate-y-0.5 group-active:shadow-none",
                ])}
              >
                ⌘ W
              </Kbd>
            </Button>
          </AppFloatingPanel>
        </PopoverContent>
      </Popover>
    </div>
  );
}
