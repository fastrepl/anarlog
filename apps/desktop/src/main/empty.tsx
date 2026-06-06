import { useCallback } from "react";

import { Kbd } from "@hypr/ui/components/ui/kbd";
import { cn } from "@hypr/utils";

import { FloatingChatCTA } from "~/shared/chat-cta";
import { StandardTabWrapper } from "~/shared/main";
import { useNewNote, useNewNoteAndListen } from "~/shared/useNewNote";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export function TabContentEmpty({
  tab: _tab,
}: {
  tab: Extract<Tab, { type: "empty" }>;
}) {
  return (
    <StandardTabWrapper floatingButton={<FloatingChatCTA />}>
      <EmptyView />
    </StandardTabWrapper>
  );
}

function EmptyView() {
  const newNote = useNewNote({ behavior: "current" });
  const newNoteAndListen = useNewNoteAndListen({ behavior: "current" });
  const openCurrent = useTabs((state) => state.openCurrent);

  const openSettings = useCallback(
    () => openCurrent({ type: "settings" }),
    [openCurrent],
  );

  return (
    <div
      data-tauri-drag-region
      className="flex h-full flex-col items-center justify-center gap-6"
    >
      <div className="flex min-w-[280px] flex-col gap-1 text-center">
        <ActionItem label="New Note" shortcut={["⌘", "N"]} onClick={newNote} />
        <ActionItem
          label="Start Recording"
          shortcut={["⌘", "⇧", "N"]}
          onClick={newNoteAndListen}
        />
        <div className="bg-accent my-1 h-px" />
        <ActionItem
          label="Settings"
          shortcut={["⌘", ","]}
          onClick={openSettings}
        />
      </div>
    </div>
  );
}

function ActionItem({
  label,
  shortcut,
  icon,
  onClick,
}: {
  label: string;
  shortcut?: string[];
  icon?: React.ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      data-tauri-drag-region="false"
      className={cn([
        "group",
        "flex items-center justify-between gap-8",
        "text-foreground text-sm",
        "rounded-full px-4 py-2",
        "hover:bg-accent cursor-pointer transition-colors",
      ])}
    >
      <span>{label}</span>
      {shortcut && shortcut.length > 0 ? (
        <Kbd
          className={cn([
            "transition-all duration-100",
            "group-hover:-translate-y-0.5 group-hover:shadow-[0_2px_0_0_var(--kbd-shadow-outer-hover),inset_0_1px_0_0_var(--kbd-shadow-inset)]",
            "group-active:translate-y-0.5 group-active:shadow-none",
          ])}
        >
          {shortcut.join(" ")}
        </Kbd>
      ) : (
        icon
      )}
    </button>
  );
}
