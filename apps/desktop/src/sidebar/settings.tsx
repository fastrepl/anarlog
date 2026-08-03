import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowsClockwise,
  ArrowUpRight,
  Bell,
  BookOpen,
  BookOpenText,
  CalendarBlank,
  Code,
  Gear,
  Lightning,
  type Icon,
  Lock,
  Sparkle,
  Sun,
  User,
  Users,
  Waveform,
} from "@phosphor-icons/react";
import { useCallback } from "react";

import { cn } from "@anlg/utils";

import { CustomSidebarHeader } from "./custom-sidebar-header";

import { type SettingsTab, useTabs } from "~/store/zustand/tabs";
import { AUTO_TEMPLATE_ID, useOpenTemplatesTab } from "~/templates";

type SettingsNavItem =
  | { id: SettingsTab; label: string; icon: Icon }
  | {
      action:
        | "open-automations"
        | "open-templates"
        | "open-calendar"
        | "open-contacts";
      label: string;
      icon: Icon;
    };

type SettingsNavGroup = { label: string; items: SettingsNavItem[] };

export function SettingsNav() {
  const { t } = useLingui();
  const currentTab = useTabs((state) => state.currentTab);
  const openNew = useTabs((state) => state.openNew);
  const updateSettingsTabState = useTabs(
    (state) => state.updateSettingsTabState,
  );
  const openTemplatesTab = useOpenTemplatesTab();

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

  const handleOpenAutomations = useCallback(() => {
    openNew({ type: "automations" });
  }, [openNew]);

  const handleOpenTemplates = useCallback(() => {
    openTemplatesTab({
      showHomepage: false,
      isWebMode: false,
      selectedMineId: AUTO_TEMPLATE_ID,
      selectedWebIndex: null,
    });
  }, [openTemplatesTab]);

  const handleOpenCalendar = useCallback(() => {
    openNew({ type: "calendar" });
  }, [openNew]);

  const handleOpenContacts = useCallback(() => {
    openNew({ type: "contacts", state: { selected: null } });
  }, [openNew]);

  const groups: SettingsNavGroup[] = [
    {
      label: t`General`,
      items: [
        { id: "app", label: t`App`, icon: Gear },
        { id: "appearance", label: t`Appearance`, icon: Sun },
        { id: "account", label: t`Account`, icon: User },
        { id: "sync", label: t`Sync`, icon: ArrowsClockwise },
        {
          action: "open-automations",
          label: t`Automations`,
          icon: Lightning,
        },
        { id: "notifications", label: t`Notifications`, icon: Bell },
        { id: "permissions", label: t`Permissions`, icon: Lock },
        { id: "developers", label: t`Developers`, icon: Code },
      ],
    },
    {
      label: t`Context`,
      items: [
        {
          action: "open-calendar",
          label: t`Calendar`,
          icon: CalendarBlank,
        },
        {
          action: "open-contacts",
          label: t`Contacts`,
          icon: Users,
        },
      ],
    },
    {
      label: "AI",
      items: [
        { id: "transcription", label: t`Transcription`, icon: Waveform },
        { id: "intelligence", label: t`Intelligence`, icon: Sparkle },
        {
          id: "dictionary",
          label: t`Dictionary`,
          icon: BookOpen,
        },
        {
          action: "open-templates",
          label: t`Templates`,
          icon: BookOpenText,
        },
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
                const isSettingsItem = "id" in item;

                return (
                  <button
                    key={isSettingsItem ? item.id : item.action}
                    onClick={() => {
                      if (!isSettingsItem) {
                        if (item.action === "open-automations") {
                          handleOpenAutomations();
                        } else if (item.action === "open-templates") {
                          handleOpenTemplates();
                        } else if (item.action === "open-calendar") {
                          handleOpenCalendar();
                        } else {
                          handleOpenContacts();
                        }
                        return;
                      }

                      setActiveTab(item.id as SettingsTab);
                    }}
                    className={cn([
                      "flex w-full items-center gap-2 rounded-full px-3 py-2 text-left text-sm",
                      "transition-colors",
                      isSettingsItem && activeTab === item.id
                        ? "bg-sidebar-accent text-foreground font-medium"
                        : "text-muted-foreground hover:bg-sidebar-accent/50 hover:text-foreground",
                    ])}
                  >
                    <item.icon
                      size={15}
                      className="shrink-0"
                      data-testid={`settings-nav-icon-${
                        isSettingsItem ? item.id : item.action
                      }`}
                    />
                    <span className="min-w-0 flex-1 truncate">
                      {item.label}
                    </span>
                    {!isSettingsItem ? (
                      <ArrowUpRight size={13} className="shrink-0" />
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
