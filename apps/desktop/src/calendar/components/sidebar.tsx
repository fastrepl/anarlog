import { useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import { useCallback, useMemo, useState, type MouseEvent } from "react";

import {
  CaretRight,
  CircleNotch,
  DotsThree,
  Plus,
} from "@anlg/ui/components/icons";
import {
  Accordion,
  AccordionContent,
  AccordionHeader,
  AccordionItem,
  AccordionTriggerPrimitive,
} from "@anlg/ui/components/ui/accordion";
import { cn } from "@anlg/utils";

import { AppleCalendarSelection } from "./apple/calendar-selection";
import {
  AppleCalendarPermissionDialog,
  TroubleShootingLink,
} from "./apple/permission";
import { type CalendarProvider, PROVIDERS } from "./shared";

import {
  allowReconnectedCalendarConnections,
  removeDisconnectedCalendarConnection,
  syncCalendarEvents,
} from "~/services/calendar";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import { usePermission } from "~/shared/hooks/usePermissions";

function getProviderBadgeClassName(badge: string) {
  if (badge === "Beta") {
    return "text-xs font-medium text-muted-foreground";
  }

  return "rounded-full border border-border px-2 text-xs font-light text-muted-foreground";
}

function getDefaultOpenProviderIds(providers: CalendarProvider[]) {
  return providers.map((provider) => provider.id);
}

function getProviderConnectionCounts() {
  return new Map<string, number>();
}

function getProviderAccordionKey(
  providers: CalendarProvider[],
  connectionCounts: Map<string, number>,
) {
  return providers
    .map(
      (provider) => `${provider.id}:${connectionCounts.get(provider.id) ?? -1}`,
    )
    .join("|");
}

function ProviderIcon({ provider }: { provider: CalendarProvider }) {
  return (
    <span className="flex size-5 shrink-0 items-center justify-center">
      {provider.icon}
    </span>
  );
}

export function CalendarSidebarContent() {
  const isMacos = platform() === "macos";
  const calendar = usePermission("calendar");
  const visibleProviders = useMemo(
    () => PROVIDERS.filter((p) => p.platform !== "macos" || isMacos),
    [isMacos],
  );
  const defaultOpenProviders = useMemo(
    () => getDefaultOpenProviderIds(visibleProviders),
    [visibleProviders],
  );
  const providerConnectionCounts = useMemo(
    () => getProviderConnectionCounts(),
    [],
  );
  const accordionKey = useMemo(
    () => getProviderAccordionKey(visibleProviders, providerConnectionCounts),
    [providerConnectionCounts, visibleProviders],
  );

  return (
    <Accordion
      key={accordionKey}
      type="multiple"
      defaultValue={defaultOpenProviders}
    >
      {visibleProviders.map((provider) =>
        provider.disabled ? (
          <div
            key={provider.id}
            className="-mx-2 flex items-center gap-2 px-2 py-3 opacity-50"
          >
            <ProviderIcon provider={provider} />
            <span className="text-sm font-medium">{provider.displayName}</span>
            {provider.badge && (
              <span className={getProviderBadgeClassName(provider.badge)}>
                {provider.badge}
              </span>
            )}
          </div>
        ) : (
          <ProviderAccordionItem
            key={provider.id}
            provider={provider}
            calendar={calendar}
          />
        ),
      )}
    </Accordion>
  );
}

function ProviderAccordionItem({
  provider,
  calendar,
}: {
  provider: CalendarProvider;
  calendar: ReturnType<typeof usePermission>;
}) {
  const { t } = useLingui();
  const [isApplePermissionDialogOpen, setIsApplePermissionDialogOpen] =
    useState(false);

  const appleNeedsPermission =
    provider.id === "apple" && calendar.status !== "authorized";

  const canDisconnectApple =
    provider.id === "apple" && calendar.status === "authorized";

  const handleAppleConnect = useCallback((): void => {
    if (calendar.isPending) return;
    allowReconnectedCalendarConnections("apple");
    if (calendar.status === "denied") {
      setIsApplePermissionDialogOpen(true);
    } else {
      calendar.request();
    }
  }, [calendar]);
  const handleAppleDisconnect = useCallback((): void => {
    void removeDisconnectedCalendarConnection("apple", "apple")
      .then(() => {
        calendar.reset();
      })
      .catch((error) => {
        console.error(
          "[calendar] failed to remove disconnected calendar data",
          error,
        );
      })
      .then(() => syncCalendarEvents())
      .catch((error) => {
        console.error("[calendar] failed to sync after disconnect", error);
      });
  }, [calendar]);
  const handleTriggerClick = useCallback(
    (event: MouseEvent<HTMLButtonElement>) => {
      if (appleNeedsPermission) {
        event.preventDefault();
        handleAppleConnect();
      }
    },
    [appleNeedsPermission, handleAppleConnect],
  );
  const providerMenuItems = useMemo(
    (): MenuItemDef[] =>
      canDisconnectApple
        ? [
            {
              id: "reconnect-apple-calendar",
              text: t`Reconnect`,
              action: () => {
                handleAppleConnect();
              },
              disabled: calendar.isPending,
            },
            {
              id: "disconnect-apple-calendar",
              text: t`Disconnect`,
              action: () => {
                handleAppleDisconnect();
              },
              disabled: calendar.isPending,
            },
          ]
        : [],
    [
      calendar.isPending,
      canDisconnectApple,
      handleAppleConnect,
      handleAppleDisconnect,
      t,
    ],
  );
  const showProviderMenu = useNativeContextMenu(providerMenuItems);
  const hasProviderMenuButton = canDisconnectApple;

  return (
    <AccordionItem value={provider.id} className="group/provider border-none">
      <div
        onContextMenu={
          providerMenuItems.length > 0 ? showProviderMenu : undefined
        }
        className={cn([
          "group/row hover:bg-accent relative -mx-2 grid items-center gap-1 rounded-full px-2",
          hasProviderMenuButton
            ? "grid-cols-[minmax(0,1fr)_auto_auto]"
            : "grid-cols-[minmax(0,1fr)_auto]",
        ])}
      >
        <AccordionHeader className="min-w-0">
          <AccordionTriggerPrimitive
            className="flex w-full min-w-0 items-center py-3 text-left text-sm font-medium transition-all hover:no-underline"
            onClick={handleTriggerClick}
          >
            <div className="flex min-w-0 items-center gap-2">
              <ProviderIcon provider={provider} />
              <span className="flex min-w-0 items-center gap-2">
                <span className="truncate text-sm font-medium">
                  {provider.displayName}
                </span>
                {provider.badge && (
                  <span className={getProviderBadgeClassName(provider.badge)}>
                    {provider.badge}
                  </span>
                )}
              </span>
            </div>
          </AccordionTriggerPrimitive>
        </AccordionHeader>

        {appleNeedsPermission ? (
          <button
            type="button"
            onClick={handleAppleConnect}
            disabled={calendar.isPending}
            className="text-muted-foreground hover:bg-accent hover:text-foreground shrink-0 rounded-full p-1 transition-colors disabled:opacity-50"
            aria-label={t`Connect ${provider.displayName}`}
          >
            {calendar.isPending ? (
              <CircleNotch className="size-4 animate-spin" />
            ) : (
              <Plus className="size-4" />
            )}
          </button>
        ) : hasProviderMenuButton ? (
          <button
            type="button"
            onClick={showProviderMenu}
            className={cn([
              "text-muted-foreground shrink-0 rounded-full p-1 transition-colors",
              "pointer-events-none opacity-0 group-hover/row:pointer-events-auto group-hover/row:opacity-100 focus-visible:pointer-events-auto focus-visible:opacity-100",
              "hover:bg-accent hover:text-muted-foreground",
            ])}
            aria-label={t`Open calendar account actions`}
          >
            <DotsThree className="size-4" />
          </button>
        ) : null}

        {!appleNeedsPermission && (
          <CaretRight
            className={cn([
              "text-muted-foreground size-4 shrink-0 transition-transform duration-200",
              "group-data-[state=open]/provider:rotate-90",
            ])}
          />
        )}
      </div>
      {!appleNeedsPermission && (
        <AccordionContent className="pb-3">
          {provider.id === "apple" && (
            <div className="flex flex-col gap-3">
              <AppleCalendarSelection
                leftAction={
                  <TroubleShootingLink
                    isPending={calendar.isPending}
                    onOpen={calendar.open}
                    onRequest={calendar.request}
                    onReset={calendar.reset}
                  />
                }
              />
            </div>
          )}
        </AccordionContent>
      )}
      {provider.id === "apple" && (
        <AppleCalendarPermissionDialog
          open={isApplePermissionDialogOpen}
          onOpenChange={setIsApplePermissionDialogOpen}
          onOpenSettings={() => void calendar.open()}
        />
      )}
    </AccordionItem>
  );
}
