import {
  AudioLinesIcon,
  BellIcon,
  BookText,
  BrainIcon,
  CalendarIcon,
  FlaskConical,
  MonitorIcon,
  SmartphoneIcon,
  SparklesIcon,
} from "lucide-react";
import { useCallback } from "react";

import { cn } from "@hypr/utils";

import { type SettingsTab, useTabs } from "~/store/zustand/tabs";

const GROUPS: {
  label: string;
  items: { id: SettingsTab; label: string; icon: typeof SmartphoneIcon }[];
}[] = [
  {
    label: "General",
    items: [
      { id: "app", label: "App", icon: SmartphoneIcon },
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
      { id: "templates", label: "Templates", icon: BookText },
      { id: "memory", label: "Memory", icon: BrainIcon },
    ],
  },
  {
    label: "Advanced",
    items: [{ id: "lab", label: "Lab", icon: FlaskConical }],
  },
];

export function SettingsNav() {
  const currentTab = useTabs((state) => state.currentTab);
  const updateSettingsTabState = useTabs(
    (state) => state.updateSettingsTabState,
  );

  const activeTab =
    currentTab?.type === "settings"
      ? currentTab.state.tab === "account"
        ? "app"
        : (currentTab.state.tab ?? "app")
      : "app";

  const setActiveTab = useCallback(
    (tab: SettingsTab) => {
      if (currentTab?.type === "settings") {
        updateSettingsTabState(currentTab, { tab });
      }
    },
    [currentTab, updateSettingsTabState],
  );

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto px-3 py-2">
      {GROUPS.map((group) => (
        <div key={group.label} className="flex flex-col gap-0.5">
          <span className="px-2 pb-1 text-[11px] font-medium tracking-wider text-neutral-400 uppercase">
            {group.label}
          </span>
          {group.items.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              onClick={() => setActiveTab(id)}
              className={cn([
                "flex items-center gap-2 rounded-md px-2 py-1.5 text-sm",
                "transition-colors",
                activeTab === id
                  ? "bg-neutral-200/70 font-medium text-neutral-900"
                  : "text-neutral-600 hover:bg-neutral-100 hover:text-neutral-800",
                id === "lab" && activeTab !== id && "text-amber-600",
                id === "lab" &&
                  activeTab === id &&
                  "bg-amber-100 text-amber-800",
              ])}
            >
              <Icon size={15} />
              <span>{label}</span>
            </button>
          ))}
        </div>
      ))}
    </div>
  );
}
