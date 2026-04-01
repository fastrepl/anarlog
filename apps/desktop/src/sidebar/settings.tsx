import { useQuery } from "@tanstack/react-query";
import { getIdentifier } from "@tauri-apps/api/app";
import {
  AudioLinesIcon,
  ArrowUpRightIcon,
  BellIcon,
  BookText,
  BrainIcon,
  CalendarIcon,
  FlaskConical,
  MonitorIcon,
  SmartphoneIcon,
  SparklesIcon,
  TicketIcon,
  TriangleAlertIcon,
  UserIcon,
} from "lucide-react";
import { useCallback } from "react";

import { cn } from "@hypr/utils";

import { type SettingsTab, useTabs } from "~/store/zustand/tabs";

const GROUPS: {
  label: string;
  items: (
    | { id: SettingsTab; label: string; icon: typeof SmartphoneIcon }
    | {
        action: "open-templates";
        label: string;
        icon: typeof SmartphoneIcon;
      }
  )[];
}[] = [
  {
    label: "General",
    items: [
      { id: "app", label: "App", icon: SmartphoneIcon },
      { id: "account", label: "Account", icon: UserIcon },
      { id: "calendar", label: "Calendar", icon: CalendarIcon },
      { id: "notifications", label: "Notifications", icon: BellIcon },
      { id: "system", label: "System", icon: MonitorIcon },
    ],
  },
  {
    label: "AI",
    items: [
      { id: "transcription", label: "Transcription", icon: AudioLinesIcon },
      { id: "intelligence", label: "Intelligence", icon: SparklesIcon },
      { id: "memory", label: "Memory", icon: BrainIcon },
      {
        action: "open-templates",
        label: "Templates",
        icon: BookText,
      },
    ],
  },
  {
    label: "Advanced",
    items: [{ id: "lab", label: "Lab", icon: FlaskConical }],
  },
];

const DONT_USE_THIS_GROUP = {
  label: "Do not use",
  items: [
    {
      id: "dont-use-this" as SettingsTab,
      label: "General",
      icon: TriangleAlertIcon,
    },
    { id: "todo" as SettingsTab, label: "Ticket", icon: TicketIcon },
  ],
};

export function SettingsNav() {
  const currentTab = useTabs((state) => state.currentTab);
  const openNew = useTabs((state) => state.openNew);
  const updateSettingsTabState = useTabs(
    (state) => state.updateSettingsTabState,
  );

  const identifierQuery = useQuery({
    queryKey: ["app-identifier"],
    queryFn: () => getIdentifier(),
    staleTime: Infinity,
  });

  const isDev = identifierQuery.data === "com.hyprnote.dev";
  const isStaging = identifierQuery.data === "com.hyprnote.staging";
  const showDontUseThis = isDev || isStaging;

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

  const groups = showDontUseThis ? [...GROUPS, DONT_USE_THIS_GROUP] : GROUPS;

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
                      if (isSettingsItem) {
                        setActiveTab(item.id);
                        return;
                      }

                      handleOpenTemplates();
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
