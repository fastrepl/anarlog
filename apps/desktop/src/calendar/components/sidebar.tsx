import { platform } from "@tauri-apps/plugin-os";
import { ChevronDown, PlusIcon } from "lucide-react";
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
    return "text-xs font-medium text-stone-600";
  }

  return "rounded-full border border-neutral-300 px-2 text-xs font-light text-neutral-500";
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

export function CalendarSidebarContent() {
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
            className="flex items-center gap-2 py-2 opacity-50"
          >
            {provider.icon}
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
        "calendar",
      );
    },
    [provider.nangoIntegrationId, shouldConnectOnClick, requiresPro],
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
        "calendar",
      );
    },
    [canAddAccount, provider.nangoIntegrationId],
  );
  const providerMenuItems = useMemo(
    () =>
      canAddAccount
        ? [
            {
              id: `add-${provider.id}-account`,
              text: `Add ${provider.displayName} account`,
              action: () =>
                void openIntegrationUrl(
                  provider.nangoIntegrationId,
                  undefined,
                  "connect",
                  "calendar",
                ),
            },
          ]
        : [],
    [
      canAddAccount,
      provider.displayName,
      provider.id,
      provider.nangoIntegrationId,
    ],
  );
  const showProviderMenu = useNativeContextMenu(providerMenuItems);

  return (
    <AccordionItem value={provider.id} className="group/provider border-none">
      <div
        onContextMenu={
          providerMenuItems.length > 0 ? showProviderMenu : undefined
        }
        className="group -mx-2 grid grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-1 rounded-md px-2 hover:bg-neutral-50"
      >
        <AccordionHeader
          className={cn(["min-w-0", requiresPro && "opacity-60"])}
        >
          <AccordionTriggerPrimitive
            className="flex w-full min-w-0 items-center py-2 text-left text-sm font-medium transition-all hover:no-underline"
            onClick={handleTriggerClick}
          >
            <div className="flex min-w-0 items-center gap-2">
              {provider.icon}
              <span className="text-sm font-medium">
                {provider.displayName}
              </span>
              {provider.badge && (
                <span className={getProviderBadgeClassName(provider.badge)}>
                  {provider.badge}
                </span>
              )}
            </div>
          </AccordionTriggerPrimitive>
        </AccordionHeader>

        {requiresPro ? (
          <button
            type="button"
            onClick={upgradeToPro}
            className="shrink-0 rounded-full border-2 border-stone-600 bg-stone-800 px-3 py-1 text-xs font-medium text-white shadow-[0_4px_14px_rgba(87,83,78,0.18)] transition-all duration-200 hover:bg-stone-700"
          >
            Upgrade to Pro
          </button>
        ) : canAddAccount ? (
          <button
            type="button"
            onClick={handleAddAccount}
            className="shrink-0 rounded p-1 text-neutral-500 transition-colors hover:bg-neutral-200 hover:text-neutral-900"
            aria-label={`Add ${provider.displayName} account`}
          >
            <PlusIcon className="size-4" />
          </button>
        ) : null}

        {!requiresPro && (
          <ChevronDown
            className={cn([
              "size-4 shrink-0 text-neutral-500 opacity-0 transition-all duration-200 group-hover:opacity-100 focus-within:opacity-100",
              "group-data-[state=open]/provider:rotate-180",
            ])}
          />
        )}
      </div>
      {!requiresPro && (
        <AccordionContent className="pb-2">
          {provider.id === "apple" && (
            <div className="flex flex-col gap-3">
              {calendar.status !== "authorized" ? (
                <AccessPermissionRow
                  title="Calendar"
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
            <OAuthProviderContent config={provider} />
          )}
        </AccordionContent>
      )}
    </AccordionItem>
  );
}
