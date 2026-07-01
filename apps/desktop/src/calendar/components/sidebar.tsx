import { useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import { ChevronRight, PlusIcon } from "lucide-react";
import { useCallback, useMemo, type MouseEvent } from "react";

import type { ConnectionItem } from "@hypr/api-client";
import {
  Accordion,
  AccordionContent,
  AccordionHeader,
  AccordionItem,
  AccordionTriggerPrimitive,
} from "@hypr/ui/components/ui/accordion";
import { cn } from "@hypr/utils";

import { AppleCalendarSelection } from "./apple/calendar-selection";
import { AccessPermissionRow, TroubleShootingLink } from "./apple/permission";
import { OAuthProviderContent } from "./oauth/provider-content";
import { type CalendarProvider, PROVIDERS } from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { useConnections } from "~/auth/useConnections";
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";
import { usePermission } from "~/shared/hooks/usePermissions";
import { openIntegrationUrl } from "~/shared/integration";

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
  const { data: connections } = useConnections(isPaid);

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
}: {
  provider: CalendarProvider;
  calendar: ReturnType<typeof usePermission>;
  returnTo: string;
}) {
  const { t } = useLingui();
  const auth = useAuth();
  const { isPaid, isPro, upgradeToPro } = useBillingAccess();
  const { data: connections, isPending, isError } = useConnections(isPaid);
  const providerConnections =
    connections?.filter(
      (connection) => connection.integration_id === provider.nangoIntegrationId,
    ) ?? [];

  const requiresPro = !!provider.nangoIntegrationId && !isPro;

  const canAddAccount =
    !!provider.nangoIntegrationId &&
    !!auth.session &&
    isPaid &&
    !isPending &&
    !isError;
  const shouldConnectOnClick =
    canAddAccount && providerConnections.length === 0;

  const handleTriggerClick = useCallback(
    (event: MouseEvent<HTMLButtonElement>) => {
      if (requiresPro) {
        event.preventDefault();
        return;
      }
      if (!shouldConnectOnClick) return;
      event.preventDefault();
      void openIntegrationUrl(
        provider.nangoIntegrationId,
        undefined,
        "connect",
        returnTo,
      );
    },
    [provider.nangoIntegrationId, requiresPro, returnTo, shouldConnectOnClick],
  );
  const handleAddAccount = useCallback(
    (event: MouseEvent<HTMLButtonElement>) => {
      if (!canAddAccount) return;
      event.preventDefault();
      event.stopPropagation();
      void openIntegrationUrl(
        provider.nangoIntegrationId,
        undefined,
        "connect",
        returnTo,
      );
    },
    [canAddAccount, provider.nangoIntegrationId, returnTo],
  );
  const handleUpgradeToPro = useCallback(
    (event: MouseEvent<HTMLButtonElement>) => {
      event.preventDefault();
      event.stopPropagation();
      upgradeToPro();
    },
    [upgradeToPro],
  );
  const providerMenuItems = useMemo(
    () =>
      canAddAccount
        ? [
            {
              id: `add-${provider.id}-account`,
              text: t`Add ${provider.displayName} account`,
              action: () =>
                void openIntegrationUrl(
                  provider.nangoIntegrationId,
                  undefined,
                  "connect",
                  returnTo,
                ),
            },
          ]
        : [],
    [
      canAddAccount,
      provider.displayName,
      provider.id,
      provider.nangoIntegrationId,
      returnTo,
      t,
    ],
  );
  const showProviderMenu = useNativeContextMenu(providerMenuItems);
  const hasAddAccountButton = canAddAccount && !requiresPro;

  return (
    <AccordionItem value={provider.id} className="group/provider border-none">
      <div
        onContextMenu={
          providerMenuItems.length > 0 ? showProviderMenu : undefined
        }
        className={cn([
          "group/row hover:bg-accent relative -mx-2 grid items-center gap-1 rounded-full px-2",
          hasAddAccountButton
            ? "grid-cols-[minmax(0,1fr)_auto_auto]"
            : "grid-cols-[minmax(0,1fr)_auto]",
        ])}
      >
        <AccordionHeader
          className={cn(["min-w-0", requiresPro && "opacity-60"])}
        >
          <AccordionTriggerPrimitive
            className="flex w-full min-w-0 items-center py-3 text-left text-sm font-medium transition-all hover:no-underline"
            onClick={handleTriggerClick}
          >
            <div className="flex min-w-0 items-center gap-2">
              <ProviderIcon provider={provider} />
              <span
                className={cn([
                  "flex min-w-0 items-center gap-2 transition-opacity duration-150",
                  requiresPro &&
                    "group-focus-within/row:opacity-0 group-hover/row:opacity-0",
                ])}
              >
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

        {requiresPro ? (
          <button
            type="button"
            onClick={handleUpgradeToPro}
            className="border-primary bg-primary text-primary-foreground hover:bg-primary/90 focus-visible:ring-ring pointer-events-none absolute top-1/2 right-1 z-10 shrink-0 translate-x-1 -translate-y-1/2 rounded-full border-2 px-3 py-1 text-xs font-medium opacity-0 shadow-[0_4px_14px_rgba(87,83,78,0.18)] transition-all duration-150 group-focus-within/row:pointer-events-auto group-focus-within/row:translate-x-0 group-focus-within/row:opacity-100 group-hover/row:pointer-events-auto group-hover/row:translate-x-0 group-hover/row:opacity-100 focus-visible:ring-2 focus-visible:outline-none"
            aria-label={t`Upgrade to Pro for ${provider.displayName}`}
          >
            {t`Upgrade to Pro`}
          </button>
        ) : hasAddAccountButton ? (
          <button
            type="button"
            onClick={handleAddAccount}
            className="text-muted-foreground hover:bg-accent hover:text-foreground shrink-0 rounded-full p-1 transition-colors"
            aria-label={t`Add ${provider.displayName} account`}
          >
            <PlusIcon className="size-4" />
          </button>
        ) : null}

        {!requiresPro && (
          <ChevronRight
            className={cn([
              "text-muted-foreground size-4 shrink-0 transition-transform duration-200",
              "group-data-[state=open]/provider:rotate-90",
            ])}
          />
        )}
      </div>
      {!requiresPro && (
        <AccordionContent className="pb-3">
          {provider.id === "apple" && (
            <div className="flex flex-col gap-3">
              {calendar.status !== "authorized" ? (
                <AccessPermissionRow
                  title={t`Calendar`}
                  status={calendar.status}
                  isPending={calendar.isPending}
                  onOpen={calendar.open}
                  onRequest={calendar.request}
                  onReset={calendar.reset}
                />
              ) : (
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
              )}
            </div>
          )}
          {provider.nangoIntegrationId && (
            <OAuthProviderContent config={provider} returnTo={returnTo} />
          )}
        </AccordionContent>
      )}
    </AccordionItem>
  );
}
