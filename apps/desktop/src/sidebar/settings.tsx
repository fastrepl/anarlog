import { platform } from "@tauri-apps/plugin-os";
import {
  AudioLinesIcon,
  ArrowUpRightIcon,
  BellIcon,
  BookText,
  CalendarIcon,
  LockIcon,
  SmartphoneIcon,
  SparklesIcon,
  UserIcon,
} from "lucide-react";
import { useCallback } from "react";

import { cn } from "@hypr/utils";

import { type SettingsTab, useTabs } from "~/store/zustand/tabs";

type SettingsNavItem =
  | { id: SettingsTab; label: string; icon: typeof SmartphoneIcon }
  | {
      action: "open-templates" | "open-calendar";
      label: string;
      icon: typeof SmartphoneIcon;
    };

type SettingsNavGroup = { label: string; items: SettingsNavItem[] };

function getBaseGroups(): SettingsNavGroup[] {
  const aiItems: SettingsNavItem[] = [
    { id: "transcription", label: "Transcription", icon: AudioLinesIcon },
    { id: "intelligence", label: "Intelligence", icon: SparklesIcon },
    {
      action: "open-templates",
      label: "Templates",
      icon: BookText,
    },
  ];

  const groups: SettingsNavGroup[] = [
    {
      label: "General",
      items: [
        { id: "app", label: "App", icon: SmartphoneIcon },
        { id: "account", label: "Account", icon: UserIcon },
        { id: "notifications", label: "Notifications", icon: BellIcon },
      ],
    },
    {
      label: "AI",
      items: aiItems,
    },
  ];

  return groups;
}

export function SettingsNav() {
  const currentTab = useTabs((state) => state.currentTab);
  const openNew = useTabs((state) => state.openNew);
  const updateSettingsTabState = useTabs(
    (state) => state.updateSettingsTabState,
  );

  const activeTab =
    currentTab?.type === "settings" ? (currentTab.state.tab ?? "app") : "app";

  const setActiveTab = useCallback(
    (tab: SettingsTab) => {
      if (currentTab?.type === "settings") {
        updateSettingsTabState(currentTab, { tab });
      }
    },
    [currentTab, updateSettingsTabState],
  );

  const handleOpenTemplates = useCallback(() => {
    openNew({ type: "templates" });
  }, [openNew]);

  const handleOpenCalendar = useCallback(() => {
    openNew({ type: "calendar" });
  }, [openNew]);

  const groups = getBaseGroups();
  const isMacos = platform() === "macos";
  if (isMacos) {
    groups[0].items.push({
      id: "permissions" as const,
      label: "Permissions",
      icon: LockIcon,
    });
  }

  groups[0].items.push({
    action: "open-calendar",
    label: "Calendar",
    icon: CalendarIcon,
  });

  return (
    <div className="flex h-full w-full flex-col overflow-hidden">
      <div className="flex h-12 items-center py-2 pr-1 pl-3">
        <h3 className="font-serif text-sm font-medium">Settings</h3>
      </div>
      <div className="scrollbar-hide flex-1 overflow-y-auto">
        <div className="flex flex-col gap-4 pb-2">
          {groups.map((group) => (
            <div key={group.label} className="flex flex-col gap-0.5">
              <span className="px-3 pb-1 text-[11px] font-medium tracking-wider text-neutral-400 uppercase">
                {group.label}
              </span>
              {group.items.map((item) => {
                const isSettingsItem = "id" in item;

                return (
                  <button
                    key={isSettingsItem ? item.id : item.action}
                    onClick={() => {
                      if (!isSettingsItem) {
                        if (item.action === "open-templates") {
                          handleOpenTemplates();
                        } else {
                          handleOpenCalendar();
                        }
                        return;
                      }

                      setActiveTab(item.id as SettingsTab);
                    }}
                    className={cn([
                      "flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-sm",
                      "transition-colors",
                      isSettingsItem && activeTab === item.id
                        ? "bg-neutral-200 font-medium text-neutral-900"
                        : "text-neutral-600 hover:bg-neutral-200/50 hover:text-neutral-800",
                    ])}
                  >
                    <item.icon size={15} />
                    <span>{item.label}</span>
                    {!isSettingsItem ? (
                      <ArrowUpRightIcon size={13} className="ml-auto" />
                    ) : null}
                  </button>
                );
              })}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
