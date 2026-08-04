import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowsClockwise,
  Bell,
  BookOpen,
  Code,
  Gear,
  type Icon,
  Lock,
  Microphone,
  Sparkle,
  Sun,
  User,
  VideoCamera,
  Waveform,
} from "@phosphor-icons/react";
import { useCallback } from "react";

import { cn } from "@anlg/utils";

import { CustomSidebarHeader } from "./custom-sidebar-header";

import { type SettingsTab, useTabs } from "~/store/zustand/tabs";

type SettingsNavItem = { id: SettingsTab; label: string; icon: Icon };

type SettingsNavGroup = { label: string; items: SettingsNavItem[] };

export function SettingsNav() {
  const { t } = useLingui();
  const currentTab = useTabs((state) => state.currentTab);
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

  const groups: SettingsNavGroup[] = [
    {
      label: t`App`,
      items: [
        { id: "app", label: t`General`, icon: Gear },
        { id: "appearance", label: t`Appearance`, icon: Sun },
        { id: "account", label: t`Account`, icon: User },
        { id: "sync", label: t`Sync`, icon: ArrowsClockwise },
        { id: "notifications", label: t`Notifications`, icon: Bell },
      ],
    },
    {
      label: t`Recording`,
      items: [
        { id: "meetings", label: t`Meetings`, icon: VideoCamera },
        { id: "audio", label: t`Audio`, icon: Microphone },
        { id: "transcription", label: t`Transcription`, icon: Waveform },
      ],
    },
    {
      label: "AI",
      items: [
        { id: "intelligence", label: t`Intelligence`, icon: Sparkle },
        {
          id: "dictionary",
          label: t`Dictionary`,
          icon: BookOpen,
        },
      ],
    },
    {
      label: t`Advanced`,
      items: [
        { id: "permissions", label: t`Permissions`, icon: Lock },
        { id: "developers", label: t`Developers`, icon: Code },
      ],
    },
  ];

  return (
    <div className="flex h-full w-full flex-col overflow-hidden">
      <CustomSidebarHeader title={<Trans>Settings</Trans>} />
      <div className="scrollbar-hide flex-1 overflow-y-auto">
        <div className="flex flex-col gap-4 pb-2">
          {groups.map((group) => (
            <div key={group.label} className="flex flex-col gap-0.5">
              <span className="text-muted-foreground px-3 pb-1 text-[11px] font-medium tracking-wider uppercase">
                {group.label}
              </span>
              {group.items.map((item) => {
                return (
                  <button
                    key={item.id}
                    onClick={() => setActiveTab(item.id)}
                    className={cn([
                      "flex w-full items-center gap-2 rounded-full px-3 py-2 text-left text-sm",
                      "transition-colors",
                      activeTab === item.id
                        ? "bg-sidebar-accent text-foreground font-medium"
                        : "text-muted-foreground hover:bg-sidebar-accent/50 hover:text-foreground",
                    ])}
                  >
                    <item.icon
                      size={15}
                      className="shrink-0"
                      data-testid={`settings-nav-icon-${item.id}`}
                    />
                    <span className="min-w-0 flex-1 truncate">
                      {item.label}
                    </span>
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
