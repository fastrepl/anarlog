import { t } from "@lingui/core/macro";
import { useCallback, useMemo } from "react";

import type { ConnectionItem } from "@anlg/api-client";
import { CircleNotch } from "@anlg/ui/components/icons";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@anlg/ui/components/ui/tooltip";

import {
  OAuthCalendarSelection,
  useOAuthCalendarSelection,
} from "./calendar-selection";
import { ReconnectRequiredIndicator } from "./status";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { useConnections } from "~/auth/useConnections";
import type { CalendarProvider } from "~/calendar/components/shared";
import { PlanGate } from "~/settings/plan-gate";
import { useOpenIntegrationUrl } from "~/shared/integration";

export function OAuthProviderContent({
  config,
  returnTo = "calendar",
  onConnectStarted,
}: {
  config: CalendarProvider;
  returnTo?: string;
  onConnectStarted?: () => void;
}) {
  const auth = useAuth();
  const { isPro } = useBillingAccess();
  const { data: connections, isError } = useConnections(isPro);
  const { openIntegration, openingAction } = useOpenIntegrationUrl();
  const providerConnections = useMemo(
    () =>
      connections?.filter(
        (c) => c.integration_id === config.nangoIntegrationId,
      ) ?? [],
    [connections, config.nangoIntegrationId],
  );

  const handleAddAccount = useCallback(() => {
    onConnectStarted?.();
    openIntegration({
      nangoIntegrationId: config.nangoIntegrationId,
      action: "connect",
      returnTo,
    });
  }, [config.nangoIntegrationId, onConnectStarted, openIntegration, returnTo]);

  if (!auth.session) {
    return (
      <div className="pt-1 pb-2">
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <span
              tabIndex={0}
              className="text-muted-foreground cursor-not-allowed text-xs opacity-50"
            >
              {t`Connect ${config.displayName} Calendar`}
            </span>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            {t`Sign in to connect your calendar`}
          </TooltipContent>
        </Tooltip>
      </div>
    );
  }

  if (!isPro) {
    return (
      <div className="pt-1 pb-2">
        <PlanGate plan="pro" allowed={false}>
          <button
            type="button"
            className="text-muted-foreground hover:text-foreground inline-flex cursor-pointer items-center gap-1 text-xs underline transition-colors"
          >
            {t`Connect ${config.displayName} Calendar`}
          </button>
        </PlanGate>
      </div>
    );
  }

  if (providerConnections.length > 0) {
    const reconnectRequired = providerConnections.filter(
      (c) => c.status === "reconnect_required",
    );

    return (
      <div className="flex flex-col gap-3 pb-2">
        {reconnectRequired.map((connection) => (
          <ReconnectRequiredContent
            key={connection.connection_id}
            config={config}
            onReconnect={() => {
              onConnectStarted?.();
              openIntegration({
                nangoIntegrationId: config.nangoIntegrationId,
                connectionId: connection.connection_id,
                action: "reconnect",
                returnTo,
              });
            }}
            onDisconnect={() =>
              openIntegration({
                nangoIntegrationId: config.nangoIntegrationId,
                connectionId: connection.connection_id,
                action: "disconnect",
                returnTo,
              })
            }
            openingAction={openingAction}
            errorDescription={connection.last_error_description ?? null}
          />
        ))}

        <ConnectedContent
          config={config}
          connections={providerConnections}
          returnTo={returnTo}
          onConnectStarted={onConnectStarted}
        />
      </div>
    );
  }

  if (isError) {
    return (
      <div className="pt-1 pb-2">
        <span className="text-xs text-red-600">
          {t`Failed to load integration status`}
        </span>
      </div>
    );
  }

  return (
    <div className="pt-1 pb-2">
      <button
        onClick={handleAddAccount}
        disabled={openingAction !== null}
        className="text-muted-foreground hover:text-foreground inline-flex cursor-pointer items-center gap-1 text-xs underline transition-colors disabled:opacity-50"
      >
        {openingAction === "connect" && (
          <CircleNotch className="size-3 animate-spin" aria-hidden="true" />
        )}
        {t`Connect ${config.displayName} Calendar`}
      </button>
    </div>
  );
}

function ReconnectRequiredContent({
  config,
  onReconnect,
  onDisconnect,
  openingAction,
  errorDescription,
}: {
  config: CalendarProvider;
  onReconnect: () => void;
  onDisconnect: () => void;
  openingAction: "connect" | "reconnect" | "disconnect" | null;
  errorDescription: string | null;
}) {
  return (
    <div className="flex flex-col gap-2 pb-2">
      <div className="flex items-center gap-2 text-xs text-amber-700">
        <ReconnectRequiredIndicator />
        <span>{t`Reconnect required for ${config.displayName} Calendar`}</span>
      </div>

      {errorDescription && (
        <p className="text-muted-foreground text-xs">{errorDescription}</p>
      )}

      <div className="flex items-center gap-2">
        <button
          onClick={onReconnect}
          disabled={openingAction !== null}
          className="text-muted-foreground hover:text-foreground inline-flex cursor-pointer items-center gap-1 text-xs underline transition-colors disabled:opacity-50"
        >
          {openingAction === "reconnect" && (
            <CircleNotch className="size-3 animate-spin" aria-hidden="true" />
          )}
          {t`Reconnect`}
        </button>
        <span className="text-muted-foreground text-xs">{t`or`}</span>
        <button
          onClick={onDisconnect}
          disabled={openingAction !== null}
          className="inline-flex cursor-pointer items-center gap-1 text-xs text-red-500 underline transition-colors hover:text-red-700 disabled:opacity-50"
        >
          {openingAction === "disconnect" && (
            <CircleNotch className="size-3 animate-spin" aria-hidden="true" />
          )}
          {t`Disconnect`}
        </button>
      </div>
    </div>
  );
}

function ConnectedContent({
  config,
  connections,
  returnTo,
  onConnectStarted,
}: {
  config: CalendarProvider;
  connections: ConnectionItem[];
  returnTo: string;
  onConnectStarted?: () => void;
}) {
  const { openIntegration } = useOpenIntegrationUrl();
  const {
    groups,
    connectionSourceMap,
    handleRefresh,
    handleToggle,
    isLoading,
  } = useOAuthCalendarSelection(config);

  const groupsWithMenus = useMemo(
    () =>
      groups.map((group) => {
        const connection = connections.find(
          (item) =>
            item.connection_id === group.id ||
            connectionSourceMap.get(item.connection_id) === group.sourceName,
        );

        if (!connection) return group;

        return {
          ...group,
          menuItems: [
            {
              id: `reconnect-${connection.connection_id}`,
              text: t`Reconnect`,
              action: () => {
                onConnectStarted?.();
                void openIntegration({
                  nangoIntegrationId: config.nangoIntegrationId,
                  connectionId: connection.connection_id,
                  action: "reconnect",
                  returnTo,
                });
              },
            },
            {
              id: `disconnect-${connection.connection_id}`,
              text: t`Disconnect`,
              action: () =>
                void openIntegration({
                  nangoIntegrationId: config.nangoIntegrationId,
                  connectionId: connection.connection_id,
                  action: "disconnect",
                  returnTo,
                }),
            },
          ],
        };
      }),
    [
      config.nangoIntegrationId,
      connectionSourceMap,
      connections,
      groups,
      onConnectStarted,
      openIntegration,
      returnTo,
    ],
  );

  return (
    <OAuthCalendarSelection
      groups={groupsWithMenus}
      onToggle={handleToggle}
      onRefresh={handleRefresh}
      isLoading={isLoading}
    />
  );
}
