import {
  CalendarIcon,
  CalendarRangeIcon,
  FolderOpenIcon,
  SettingsIcon,
  StickyNoteIcon,
  UsersIcon,
  X,
} from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { type Tab, uniqueIdFromTab } from "~/tabs/tabs.types";

export function TabItemView({
  tab,
  selected,
  title,
  shortcutIndex,
  onSelect,
  onClose,
}: {
  tab: Tab;
  selected: boolean;
  title: string;
  shortcutIndex?: number;
  onSelect: () => void;
  onClose: () => void;
}) {
  const Icon = iconForTab(tab);

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onSelect();
        }
      }}
      className={cn([
        "group relative flex h-8 items-center gap-2 rounded-lg border px-3",
        "transition-colors",
        selected
          ? "border-stone-400 bg-neutral-200/50 text-black"
          : "border-transparent bg-neutral-50 text-neutral-500 hover:bg-stone-100",
      ])}
      title={title}
      data-tab-id={uniqueIdFromTab(tab)}
    >
      <Icon size={14} className="shrink-0" />
      <span className="max-w-40 truncate text-sm">{title}</span>
      {shortcutIndex ? (
        <span className="text-[10px] text-neutral-400">{shortcutIndex}</span>
      ) : null}
      <Button
        type="button"
        variant="ghost"
        size="icon"
        className={cn([
          "ml-1 size-5 shrink-0 text-neutral-400 opacity-0",
          "group-hover:opacity-100 hover:text-neutral-700",
          selected && "opacity-100",
        ])}
        onClick={(event) => {
          event.stopPropagation();
          onClose();
        }}
        title={`Close ${title}`}
      >
        <X size={12} />
      </Button>
    </div>
  );
}

function iconForTab(tab: Tab) {
  switch (tab.type) {
    case "sessions":
      return StickyNoteIcon;
    case "settings":
      return SettingsIcon;
    case "folders":
      return FolderOpenIcon;
    case "contacts":
      return UsersIcon;
    case "calendar":
      return CalendarIcon;
    case "daily-summary":
      return CalendarRangeIcon;
  }
}
