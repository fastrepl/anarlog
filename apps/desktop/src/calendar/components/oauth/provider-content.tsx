import { useCallback, useMemo } from "react";

import type { ConnectionItem } from "@hypr/api-client";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";

import {
  OAuthCalendarSelection,
  useOAuthCalendarSelection,
} from "./calendar-selection";
import { ReconnectRequiredIndicator } from "./status";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { useConnections } from "~/auth/useConnections";
import type { CalendarProvider } from "~/calendar/components/shared";
import { openIntegrationUrl } from "~/shared/integration";

export function OAuthProviderContent({ config }: { config: CalendarProvider }) {
  const auth = useAuth();
  const { isPro, upgradeToPro } = useBillingAccess();
  const { data: connections, isError } = useConnections(isPro);
  const providerConnections = useMemo(
    () =>
      connections?.filter(
        (c) => c.integration_id === config.nangoIntegrationId,
      ) ?? [],
    [connections, config.nangoIntegrationId],
  );

  const handleAddAccount = useCallback(
    () =>
      openIntegrationUrl(
        config.nangoIntegrationId,
        undefined,
        "connect",
        "calendar",
      ),
    [config.nangoIntegrationId],
  );

  if (!auth.session) {
    return (
      <div className="pt-1 pb-2">
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <span
              tabIndex={0}
              className="cursor-not-allowed text-xs text-neutral-400 opacity-50"
            >
              Connect {config.displayName} Calendar
            </span>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            Sign in to connect your calendar
          </TooltipContent>
        </Tooltip>
      </div>
    );
  }

  if (!isPro) {
    return (
      <div className="pt-1 pb-2">
        <button
          onClick={upgradeToPro}
          className="cursor-pointer text-xs text-neutral-600 underline transition-colors hover:text-neutral-900"
        >
          Upgrade to connect
        </button>
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
            onReconnect={() =>
              openIntegrationUrl(
                config.nangoIntegrationId,
                connection.connection_id,
                "reconnect",
                "calendar",
              )
            }
            onDisconnect={() =>
              openIntegrationUrl(
                config.nangoIntegrationId,
                connection.connection_id,
                "disconnect",
                "calendar",
              )
            }
            errorDescription={connection.last_error_description ?? null}
          />
        ))}

        <ConnectedContent config={config} connections={providerConnections} />
      </div>
    );
  }

  if (isError) {
    return (
      <div className="pt-1 pb-2">
        <span className="text-xs text-red-600">
          Failed to load integration status
        </span>
      </div>
    );
  }

  return (
    <div className="pt-1 pb-2">
      <button
        onClick={handleAddAccount}
        className="cursor-pointer text-xs text-neutral-600 underline transition-colors hover:text-neutral-900"
      >
        Connect {config.displayName} Calendar
      </button>
    </div>
  );
}

function ReconnectRequiredContent({
  config,
  onReconnect,
  onDisconnect,
  errorDescription,
}: {
  config: CalendarProvider;
  onReconnect: () => void;
  onDisconnect: () => void;
  errorDescription: string | null;
}) {
  return (
    <div className="flex flex-col gap-2 pb-2">
      <div className="flex items-center gap-2 text-xs text-amber-700">
        <ReconnectRequiredIndicator />
        <span>Reconnect required for {config.displayName} Calendar</span>
      </div>

      {errorDescription && (
        <p className="text-xs text-neutral-600">{errorDescription}</p>
      )}

      <div className="flex items-center gap-2">
        <button
          onClick={onReconnect}
          className="cursor-pointer text-xs text-neutral-600 underline transition-colors hover:text-neutral-900"
        >
          Reconnect
        </button>
        <span className="text-xs text-neutral-400">or</span>
        <button
          onClick={onDisconnect}
          className="cursor-pointer text-xs text-red-500 underline transition-colors hover:text-red-700"
        >
          Disconnect
        </button>
      </div>
    </div>
  );
}

function ConnectedContent({
  config,
  connections,
}: {
  config: CalendarProvider;
  connections: ConnectionItem[];
}) {
  const { groups, connectionSourceMap, handleToggle, isLoading } =
    useOAuthCalendarSelection(config);

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
              text: "Reconnect",
              action: () =>
                void openIntegrationUrl(
                  config.nangoIntegrationId,
                  connection.connection_id,
                  "reconnect",
                  "calendar",
                ),
            },
            {
              id: `disconnect-${connection.connection_id}`,
              text: "Disconnect",
              action: () =>
                void openIntegrationUrl(
                  config.nangoIntegrationId,
                  connection.connection_id,
                  "disconnect",
                  "calendar",
                ),
            },
          ],
        };
      }),
    [config.nangoIntegrationId, connectionSourceMap, connections, groups],
  );

  return (
    <OAuthCalendarSelection
      groups={groupsWithMenus}
      onToggle={handleToggle}
      isLoading={isLoading}
    />
  );
}
