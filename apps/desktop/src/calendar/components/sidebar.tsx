import { useLingui } from "@lingui/react/macro";
import {
  CaretRight,
  CircleNotch,
  DotsThree,
  Plus,
} from "@phosphor-icons/react";
import { platform } from "@tauri-apps/plugin-os";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent,
} from "react";

import type { ConnectionItem } from "@anlg/api-client";
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
import { OAuthProviderContent } from "./oauth/provider-content";
import {
  type CalendarProvider,
  getCalendarConnectionKey,
  PROVIDERS,
} from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { useConnections } from "~/auth/useConnections";
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
import { useOpenIntegrationUrl } from "~/shared/integration";

function getProviderBadgeClassName(badge: string) {
  if (badge === "Beta") {
    return "text-xs font-medium text-muted-foreground";
  }

  return "rounded-full border border-border px-2 text-xs font-light text-muted-foreground";
}

function getDefaultOpenProviderIds(
  providers: CalendarProvider[],
  connections: ConnectionItem[] | undefined,
) {
  return providers
    .filter(
      (provider) =>
        !provider.nangoIntegrationId ||
        connections?.some(
          (connection) =>
            connection.integration_id === provider.nangoIntegrationId,
        ),
    )
    .map((provider) => provider.id);
}

function getProviderConnectionCounts(
  providers: CalendarProvider[],
  connections: ConnectionItem[] | undefined,
) {
  return new Map(
    providers
      .filter((provider) => provider.nangoIntegrationId)
      .map((provider) => [
        provider.id,
        connections?.filter(
          (connection) =>
            connection.integration_id === provider.nangoIntegrationId,
        ).length ?? 0,
      ]),
  );
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

const CONNECTION_POLL_MS = 45_000;
const CONNECTION_POLL_INTERVAL_MS = 1_500;

function ProviderIcon({ provider }: { provider: CalendarProvider }) {
  return (
    <span className="flex size-5 shrink-0 items-center justify-center">
      {provider.icon}
    </span>
  );
}

export function CalendarSidebarContent({
  returnTo = "calendar",
}: {
  returnTo?: string;
}) {
  const isMacos = platform() === "macos";
  const calendar = usePermission("calendar");
  const { isPaid } = useBillingAccess();
  const [connectionPollUntil, setConnectionPollUntil] = useState<number | null>(
    null,
  );
  const connectionKeyWhenPollStartedRef = useRef("");
  const isPollingConnections = connectionPollUntil !== null;
  const { data: connections } = useConnections(isPaid, {
    refetchInterval: isPollingConnections ? CONNECTION_POLL_INTERVAL_MS : false,
  });
  const connectionKey = getCalendarConnectionKey(connections);
  const watchForNewConnection = useCallback(() => {
    connectionKeyWhenPollStartedRef.current = connectionKey;
    setConnectionPollUntil(Date.now() + CONNECTION_POLL_MS);
  }, [connectionKey]);

  useEffect(() => {
    if (connectionPollUntil === null) {
      return;
    }
    const remaining = connectionPollUntil - Date.now();
    if (remaining <= 0) {
      setConnectionPollUntil(null);
      return;
    }
    const timeoutId = window.setTimeout(() => {
      setConnectionPollUntil(null);
    }, remaining);
    return () => window.clearTimeout(timeoutId);
  }, [connectionPollUntil]);

  useEffect(() => {
    if (
      !isPollingConnections ||
      connectionKey === connectionKeyWhenPollStartedRef.current
    ) {
      return;
    }
    setConnectionPollUntil(null);
  }, [connectionKey, isPollingConnections]);

  const visibleProviders = useMemo(
    () =>
      PROVIDERS.filter(
        (p) => p.platform === "all" || (p.platform === "macos" && isMacos),
      ),
    [isMacos],
  );
  const defaultOpenProviders = useMemo(
    () => getDefaultOpenProviderIds(visibleProviders, connections),
    [connections, visibleProviders],
  );
  const providerConnectionCounts = useMemo(
    () => getProviderConnectionCounts(visibleProviders, connections),
    [connections, visibleProviders],
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
            returnTo={returnTo}
            onConnectStarted={watchForNewConnection}
          />
        ),
      )}
    </Accordion>
  );
}

function ProviderAccordionItem({
  provider,
  calendar,
  returnTo,
  onConnectStarted,
}: {
  provider: CalendarProvider;
  calendar: ReturnType<typeof usePermission>;
  returnTo: string;
  onConnectStarted: () => void;
}) {
  const { t } = useLingui();
  const auth = useAuth();
  const { isPaid, isPro } = useBillingAccess();
  const { openIntegration, openingAction } = useOpenIntegrationUrl();
  const { data: connections, isPending, isError } = useConnections(isPaid);
  const [isApplePermissionDialogOpen, setIsApplePermissionDialogOpen] =
    useState(false);
  const providerConnections =
    connections?.filter(
      (connection) => connection.integration_id === provider.nangoIntegrationId,
    ) ?? [];

  const requiresPro = !!provider.nangoIntegrationId && !isPro;
  const appleNeedsPermission =
    provider.id === "apple" && calendar.status !== "authorized";

  const canAddAccount =
    !!provider.nangoIntegrationId &&
    !!auth.session &&
    isPaid &&
    !isPending &&
    !isError;
  const shouldConnectOnClick =
    canAddAccount && providerConnections.length === 0;

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
        return;
      }
      if (!shouldConnectOnClick) return;
      event.preventDefault();
      onConnectStarted();
      openIntegration({
        nangoIntegrationId: provider.nangoIntegrationId,
        action: "connect",
        returnTo,
      });
    },
    [
      appleNeedsPermission,
      handleAppleConnect,
      onConnectStarted,
      openIntegration,
      provider.nangoIntegrationId,
      returnTo,
      shouldConnectOnClick,
    ],
  );
  const handleAddAccount = useCallback(
    (event: MouseEvent<HTMLButtonElement>) => {
      if (!canAddAccount) return;
      event.preventDefault();
      event.stopPropagation();
      onConnectStarted();
      openIntegration({
        nangoIntegrationId: provider.nangoIntegrationId,
        action: "connect",
        returnTo,
      });
    },
    [
      canAddAccount,
      onConnectStarted,
      openIntegration,
      provider.nangoIntegrationId,
      returnTo,
    ],
  );
  const providerMenuItems = useMemo(
    (): MenuItemDef[] =>
      canAddAccount
        ? [
            {
              id: `add-${provider.id}-account`,
              text: t`Add ${provider.displayName} account`,
              action: () => {
                onConnectStarted();
                void openIntegration({
                  nangoIntegrationId: provider.nangoIntegrationId,
                  action: "connect",
                  returnTo,
                });
              },
            },
          ]
        : canDisconnectApple
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
      canAddAccount,
      canDisconnectApple,
      handleAppleConnect,
      handleAppleDisconnect,
      onConnectStarted,
      provider.displayName,
      provider.id,
      provider.nangoIntegrationId,
      openIntegration,
      returnTo,
      t,
    ],
  );
  const showProviderMenu = useNativeContextMenu(providerMenuItems);
  const hasAddAccountButton = canAddAccount && !requiresPro;
  const hasProviderMenuButton = canDisconnectApple;

  return (
    <AccordionItem value={provider.id} className="group/provider border-none">
      <div
        onContextMenu={
          providerMenuItems.length > 0 ? showProviderMenu : undefined
        }
        className={cn([
          "group/row hover:bg-accent relative -mx-2 grid items-center gap-1 rounded-full px-2",
          hasAddAccountButton || hasProviderMenuButton
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
        ) : hasAddAccountButton ? (
          <button
            type="button"
            onClick={handleAddAccount}
            disabled={openingAction !== null}
            className="text-muted-foreground hover:bg-accent hover:text-foreground shrink-0 rounded-full p-1 transition-colors disabled:opacity-50"
            aria-label={t`Add ${provider.displayName} account`}
          >
            {openingAction === "connect" ? (
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
          {provider.nangoIntegrationId && (
            <OAuthProviderContent
              config={provider}
              returnTo={returnTo}
              onConnectStarted={onConnectStarted}
            />
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
