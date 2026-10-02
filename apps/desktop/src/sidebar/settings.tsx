import { Trans, useLingui } from "@lingui/react/macro";
import { useCallback, useState } from "react";

import { Lock, MagnifyingGlass, X } from "@anlg/ui/components/icons";
import { useSquircleRef } from "@anlg/ui/hooks/use-squircle";
import { cn } from "@anlg/utils";

import { CustomSidebarHeader } from "./custom-sidebar-header";
import { useSettingsNavGroups } from "./settings-nav-groups";

import { useBillingAccess } from "~/auth/billing-context";
import { type SettingsTab, useTabs } from "~/store/zustand/tabs";

export function SettingsNav() {
  const { t } = useLingui();
  const { isPro } = useBillingAccess();
  const groups = useSettingsNavGroups();
  const [search, setSearch] = useState("");
  const searchRef = useSquircleRef<HTMLDivElement>();
  const currentTab = useTabs((state) => state.currentTab);
  const updateSettingsTabState = useTabs(
    (state) => state.updateSettingsTabState,
  );
  const openNew = useTabs((state) => state.openNew);

  const requestedTab =
    currentTab?.type === "settings" ? (currentTab.state.tab ?? "app") : "app";
  const activeTab =
    requestedTab === "audio"
      ? "meetings"
      : requestedTab === "stats"
        ? "insights"
        : requestedTab;

  const setActiveTab = useCallback(
    (tab: SettingsTab) => {
      if (currentTab?.type === "settings") {
        updateSettingsTabState(currentTab, { tab });
      }
    },
    [currentTab, updateSettingsTabState],
  );

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
          ref={searchRef}
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
        <div className="flex flex-col gap-5 pb-6">
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
                  <div key={item.id} className="relative">
                    <button
                      type="button"
                      onClick={() => {
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
                      ])}
                    >
                      <item.icon
                        size={15}
                        className="shrink-0"
                        data-testid={`settings-nav-icon-${item.id}`}
                      />
                      <span className="flex min-w-0 flex-1 items-center gap-2">
                        <span className="min-w-0 flex-1 truncate">
                          {item.label}
                        </span>
                        {requiresPro ? (
                          <Lock
                            aria-label={t`Requires Anarlog Pro`}
                            className="size-3.5 shrink-0"
                          />
                        ) : null}
                      </span>
                    </button>
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
