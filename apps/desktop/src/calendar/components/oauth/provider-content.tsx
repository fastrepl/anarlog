import { RefreshCwIcon } from "lucide-react";
import { useCallback } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Button } from "@hypr/ui/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import {
  OAuthCalendarSelection,
  useOAuthCalendarSelection,
} from "./calendar-selection";
import {
  ConnectionTroubleShootingLink,
  ReconnectRequiredIndicator,
} from "./status";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { useConnections } from "~/auth/useConnections";
import type { CalendarProvider } from "~/calendar/components/shared";
import { buildWebAppUrl } from "~/shared/utils";

export function OAuthProviderContent({ config }: { config: CalendarProvider }) {
  const auth = useAuth();
  const { isPro, upgradeToPro } = useBillingAccess();
  const { data: connections, isError } = useConnections(isPro);
  const connection = connections?.find(
    (c) => c.integration_id === config.nangoIntegrationId,
  );
  const connectionNeedsReconnect = connection?.status === "reconnect_required";

  const handleConnect = useCallback(
    () =>
      openIntegrationUrl(
        config.nangoIntegrationId,
        connection?.connection_id,
        "connect",
      ),
    [config.nangoIntegrationId, connection?.connection_id],
  );

  const handleDisconnect = useCallback(
    () =>
      openIntegrationUrl(
        config.nangoIntegrationId,
        connection?.connection_id,
        "disconnect",
      ),
    [config.nangoIntegrationId, connection?.connection_id],
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
          Upgrade to Pro to connect
        </button>
      </div>
    );
  }

  if (connection) {
    if (connectionNeedsReconnect) {
      return (
        <ReconnectRequiredContent
          config={config}
          onConnect={handleConnect}
          onDisconnect={handleDisconnect}
          errorDescription={connection.last_error_description ?? null}
        />
      );
    }

    return (
      <ConnectedContent
        config={config}
        onConnect={handleConnect}
        onDisconnect={handleDisconnect}
      />
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
        onClick={handleConnect}
        className="cursor-pointer text-xs text-neutral-600 underline transition-colors hover:text-neutral-900"
      >
        Connect {config.displayName} Calendar
      </button>
    </div>
  );
}

function ReconnectRequiredContent({
  config,
  onConnect,
  onDisconnect,
  errorDescription,
}: {
  config: CalendarProvider;
  onConnect: () => void;
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
          onClick={onConnect}
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
  onConnect,
  onDisconnect,
}: {
  config: CalendarProvider;
  onConnect: () => void;
  onDisconnect: () => void;
}) {
  const { groups, handleToggle, handleRefresh, isLoading } =
    useOAuthCalendarSelection(config);

  return (
    <div className="flex flex-col gap-2 pb-2">
      <div className="flex items-center justify-between">
        <ConnectionTroubleShootingLink
          onConnect={onConnect}
          onDisconnect={onDisconnect}
        />

        <Button
          variant="ghost"
          size="icon"
          onClick={handleRefresh}
          className="size-6"
          disabled={isLoading}
        >
          <RefreshCwIcon
            className={cn(["size-3.5", isLoading && "animate-spin"])}
          />
        </Button>
      </div>

      <OAuthCalendarSelection
        groups={groups}
        onToggle={handleToggle}
        isLoading={isLoading}
      />
    </div>
  );
}

async function openIntegrationUrl(
  nangoIntegrationId: string | undefined,
  connectionId: string | undefined,
  action: "connect" | "disconnect",
) {
  if (!nangoIntegrationId) return;
  const params: Record<string, string> = {
    action,
    integration_id: nangoIntegrationId,
    return_to: "calendar",
  };
  if (action === "disconnect" && connectionId) {
    params.connection_id = connectionId;
  }
  const url = await buildWebAppUrl("/app/integration", params);
  await openerCommands.openUrl(url, null);
}
