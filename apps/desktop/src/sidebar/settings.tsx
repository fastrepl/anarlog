import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowUpRight,
  ArrowsClockwise,
  Bell,
  BookOpen,
  CalendarDots,
  CircleNotch,
  Code,
  DownloadSimple,
  FileText,
  FolderSimple,
  Gear,
  Lightning,
  type Icon,
  Lock,
  MagnifyingGlass,
  ShieldCheck,
  Sparkle,
  Sun,
  User,
  Users,
  UsersThree,
  VideoCamera,
  X,
} from "@phosphor-icons/react";
import { useCallback, useState } from "react";

import { cn } from "@anlg/utils";

import { CustomSidebarHeader } from "./custom-sidebar-header";

import { useBillingAccess } from "~/auth/billing-context";
import { privacyMessages } from "~/settings/general/app-settings";
import { useMyWorkspacesWithMirror } from "~/settings/team/mirror";
import { type SettingsTab, type TabInput, useTabs } from "~/store/zustand/tabs";

type SettingsNavItem =
  | {
      id: SettingsTab;
      label: string;
      icon: Icon;
      requiresPro?: boolean;
    }
  | {
      id: "automations" | "calendar" | "contacts" | "folders" | "templates";
      label: string;
      icon: Icon;
      destination: TabInput;
      requiresPro?: boolean;
    };

type SettingsNavGroup = { label: string; items: SettingsNavItem[] };

export function SettingsNav() {
  const { i18n, t } = useLingui();
  const { isPro, upgradeToPro, isUpgradingToPro } = useBillingAccess();
  const workspaces = useMyWorkspacesWithMirror();
  const hasExistingWorkspace = (workspaces.data?.length ?? 0) > 0;
  const [search, setSearch] = useState("");
  const currentTab = useTabs((state) => state.currentTab);
  const updateSettingsTabState = useTabs(
    (state) => state.updateSettingsTabState,
  );
  const openNew = useTabs((state) => state.openNew);

  const requestedTab =
    currentTab?.type === "settings" ? (currentTab.state.tab ?? "app") : "app";
  const activeTab = requestedTab === "audio" ? "meetings" : requestedTab;

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
        { id: "account", label: t`Account`, icon: User },
        {
          id: "team",
          label: t`Teams`,
          icon: UsersThree,
          requiresPro: !workspaces.isLoading && !hasExistingWorkspace,
        },
        { id: "appearance", label: t`Appearance`, icon: Sun },
        { id: "notifications", label: t`Notifications`, icon: Bell },
      ],
    },
    {
      label: t`Workspace`,
      items: [
        { id: "meetings", label: t`Meetings`, icon: VideoCamera },
        {
          id: "folders",
          label: t`Folders`,
          icon: FolderSimple,
          destination: { type: "folders" },
        },
        {
          id: "calendar",
          label: t`Calendar`,
          icon: CalendarDots,
          destination: { type: "calendar" },
        },
        {
          id: "contacts",
          label: t`Contacts`,
          icon: Users,
          destination: { type: "contacts" },
        },
        {
          id: "templates",
          label: t`Templates`,
          icon: FileText,
          destination: { type: "templates" },
        },
        {
          id: "automations",
          label: t`Automations`,
          icon: Lightning,
          destination: { type: "automations" },
          requiresPro: true,
        },
      ],
    },
    {
      label: "AI",
      items: [
        { id: "transcription", label: t`Transcription`, icon: Sparkle },
        { id: "intelligence", label: t`Intelligence`, icon: Sparkle },
        {
          id: "dictionary",
          label: t`Dictionary`,
          icon: BookOpen,
          requiresPro: true,
        },
      ],
    },
    {
      label: t`Data`,
      items: [
        {
          id: "sync",
          label: t`Sync`,
          icon: ArrowsClockwise,
          requiresPro: true,
        },
        { id: "imports", label: t`Imports`, icon: DownloadSimple },
      ],
    },
    {
      label: t`Advanced`,
      items: [
        {
          id: "privacy",
          label: i18n._(privacyMessages.title),
          icon: ShieldCheck,
        },
        { id: "permissions", label: t`Permissions`, icon: Lock },
        { id: "developers", label: t`Developers`, icon: Code },
      ],
    },
  ];

  const query = search.trim().toLowerCase();
  const visibleGroups = query
    ? groups
        .map((group) =>
          group.label.toLowerCase().includes(query)
            ? group
            : {
                ...group,
                items: group.items.filter((item) =>
                  item.label.toLowerCase().includes(query),
                ),
              },
        )
        .filter((group) => group.items.length > 0)
    : groups;

  return (
    <div className="flex h-full w-full flex-col overflow-hidden">
      <CustomSidebarHeader />
      <div className="pb-2">
        <div
          className={cn([
            "border-border bg-accent/50 flex h-8 w-full shrink-0 items-center gap-2 rounded-lg border px-3",
            "focus-within:bg-accent transition-colors",
          ])}
        >
          <MagnifyingGlass className="text-muted-foreground h-4 w-4 shrink-0" />
          <input
            type="text"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                setSearch("");
              }
            }}
            placeholder={t`Search settings...`}
            className="placeholder:text-muted-foreground min-w-0 flex-1 bg-transparent text-sm placeholder:text-sm focus:outline-hidden"
          />
          {search ? (
            <button
              type="button"
              onClick={() => setSearch("")}
              className={cn([
                "size-4 shrink-0",
                "text-muted-foreground hover:text-foreground",
                "transition-colors",
              ])}
              aria-label={t`Clear search`}
            >
              <X className="size-4" />
            </button>
          ) : null}
        </div>
      </div>
      <div className="scrollbar-hide flex-1 overflow-y-auto">
        <div className="flex flex-col gap-5 pb-2">
          {visibleGroups.length === 0 ? (
            <div className="text-muted-foreground px-3 py-8 text-center">
              <MagnifyingGlass
                size={32}
                className="text-muted-foreground/70 mx-auto mb-2"
              />
              <p className="text-sm">
                <Trans>No results found.</Trans>
              </p>
            </div>
          ) : null}
          {visibleGroups.map((group) => (
            <div key={group.label} className="flex flex-col gap-0.5">
              <span className="text-muted-foreground/60 px-3 pb-1 text-[11px] font-medium tracking-wider uppercase">
                {group.label}
              </span>
              {group.items.map((item) => {
                const requiresPro = Boolean(item.requiresPro && !isPro);

                return (
                  <div key={item.id} className="group/row relative">
                    <button
                      type="button"
                      aria-disabled={requiresPro}
                      onClick={() => {
                        if (requiresPro) return;

                        if ("destination" in item) {
                          openNew(item.destination);
                          return;
                        }

                        setActiveTab(item.id);
                      }}
                      className={cn([
                        "flex w-full items-center gap-2 rounded-full px-3 py-2 text-left text-sm",
                        "transition-colors",
                        activeTab === item.id
                          ? "bg-sidebar-accent text-foreground font-medium"
                          : "text-muted-foreground hover:bg-sidebar-accent/50 hover:text-foreground",
                        requiresPro && "opacity-60",
                      ])}
                    >
                      <item.icon
                        size={15}
                        className="shrink-0"
                        data-testid={`settings-nav-icon-${item.id}`}
                      />
                      <span
                        className={cn([
                          "flex min-w-0 flex-1 items-center gap-2 transition-opacity duration-150",
                          requiresPro &&
                            "group-focus-within/row:opacity-0 group-hover/row:opacity-0",
                        ])}
                      >
                        <span className="min-w-0 flex-1 truncate">
                          {item.label}
                        </span>
                        {requiresPro ? (
                          <Lock aria-hidden className="size-3.5 shrink-0" />
                        ) : "destination" in item ? (
                          <ArrowUpRight
                            aria-hidden
                            className="text-muted-foreground/70 size-3.5 shrink-0"
                            data-testid={`settings-nav-destination-icon-${item.id}`}
                          />
                        ) : null}
                      </span>
                    </button>
                    {requiresPro ? (
                      <button
                        type="button"
                        onClick={upgradeToPro}
                        disabled={isUpgradingToPro}
                        className="border-primary bg-primary text-primary-foreground hover:bg-primary/90 focus-visible:ring-ring pointer-events-none absolute top-1/2 right-1 flex translate-x-1 -translate-y-1/2 items-center gap-1 rounded-full border-2 px-3 py-1 text-xs font-medium opacity-0 shadow-[0_4px_14px_rgba(87,83,78,0.18)] transition-all duration-150 group-focus-within/row:pointer-events-auto group-focus-within/row:translate-x-0 group-focus-within/row:opacity-100 group-hover/row:pointer-events-auto group-hover/row:translate-x-0 group-hover/row:opacity-100 focus-visible:ring-2 focus-visible:outline-none disabled:opacity-70"
                        aria-label={t`Upgrade to Pro for ${item.label}`}
                      >
                        {isUpgradingToPro ? (
                          <CircleNotch
                            className="size-3 animate-spin"
                            aria-hidden
                          />
                        ) : null}
                        <Trans>Upgrade to Pro</Trans>
                      </button>
                    ) : null}
                  </div>
                );
              })}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
