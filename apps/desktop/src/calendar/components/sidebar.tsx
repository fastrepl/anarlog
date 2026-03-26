import { platform } from "@tauri-apps/plugin-os";
import { ChevronDown, PlusIcon } from "lucide-react";
import { useCallback, type MouseEvent, useMemo } from "react";

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
import {
  OAuthProviderContent,
  openIntegrationUrl,
} from "./oauth/provider-content";
import { type CalendarProvider, PROVIDERS } from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { useConnections } from "~/auth/useConnections";
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";
import { usePermission } from "~/shared/hooks/usePermissions";

function getProviderBadgeClassName(badge: string) {
  return cn([
    "rounded-full px-2 text-xs",
    badge === "Beta"
      ? "bg-sky-100 py-0.5 font-medium text-sky-900"
      : "border border-neutral-300 font-light text-neutral-500",
  ]);
}

export function CalendarSidebarContent() {
  const isMacos = platform() === "macos";
  const calendar = usePermission("calendar");

  const visibleProviders = PROVIDERS.filter(
    (p) => p.platform === "all" || (p.platform === "macos" && isMacos),
  );

  return (
    <Accordion type="multiple" defaultValue={["apple"]}>
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
  const { isPro } = useBillingAccess();
  const { data: connections, isPending, isError } = useConnections(isPro);
  const providerConnections =
    connections?.filter(
      (connection) => connection.integration_id === provider.nangoIntegrationId,
    ) ?? [];
  const canAddAccount =
    !!provider.nangoIntegrationId &&
    !!auth.session &&
    isPro &&
    !isPending &&
    !isError;
  const shouldConnectOnClick =
    canAddAccount && providerConnections.length === 0;

  const handleTriggerClick = useCallback(
    (event: MouseEvent<HTMLButtonElement>) => {
      if (!shouldConnectOnClick) return;
      event.preventDefault();
      void openIntegrationUrl(
        provider.nangoIntegrationId,
        undefined,
        "connect",
      );
    },
    [provider.nangoIntegrationId, shouldConnectOnClick],
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
        <AccordionHeader className="min-w-0">
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

        {canAddAccount && (
          <button
            type="button"
            onClick={handleAddAccount}
            className="shrink-0 rounded p-1 text-neutral-500 transition-colors hover:bg-neutral-200 hover:text-neutral-900"
            aria-label={`Add ${provider.displayName} account`}
          >
            <PlusIcon className="size-4" />
          </button>
        )}

        <ChevronDown
          className={cn([
            "size-4 shrink-0 text-neutral-500 opacity-0 transition-all duration-200 group-hover:opacity-100 focus-within:opacity-100",
            "group-data-[state=open]/provider:rotate-180",
          ])}
        />
      </div>
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
    </AccordionItem>
  );
}
