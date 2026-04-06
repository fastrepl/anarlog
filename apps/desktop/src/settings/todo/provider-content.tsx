import { useCallback, useMemo, useState } from "react";

import { Input } from "@hypr/ui/components/ui/input";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";

import type { TodoProvider } from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { useConnections } from "~/auth/useConnections";
import { openIntegrationUrl } from "~/shared/integration";

export function TodoProviderContent({ config }: { config: TodoProvider }) {
  const auth = useAuth();
  const { isPaid, upgradeToPro } = useBillingAccess();
  const { data: connections, isError } = useConnections(isPaid);
  const [filter, setFilter] = useState("");

  const providerConnections = useMemo(
    () =>
      connections?.filter(
        (c) => c.integration_id === config.nangoIntegrationId,
      ) ?? [],
    [connections, config.nangoIntegrationId],
  );

  const handleConnect = useCallback(
    () =>
      openIntegrationUrl(
        config.nangoIntegrationId,
        undefined,
        "connect",
        "todo",
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
              Connect {config.displayName}
            </span>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            Sign in to connect {config.displayName}
          </TooltipContent>
        </Tooltip>
      </div>
    );
  }

  if (!isPaid) {
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

  if (isError) {
    return (
      <div className="pt-1 pb-2">
        <span className="text-xs text-red-600">
          Failed to load integration status
        </span>
      </div>
    );
  }

  if (providerConnections.length === 0) {
    return (
      <div className="pt-1 pb-2">
        <button
          onClick={handleConnect}
          className="cursor-pointer text-xs text-neutral-600 underline transition-colors hover:text-neutral-900"
        >
          Connect {config.displayName}
        </button>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-2 pb-1">
        {providerConnections.some((c) => c.status === "reconnect_required") ? (
          <div className="flex items-center gap-2">
            <button
              onClick={() =>
                openIntegrationUrl(
                  config.nangoIntegrationId,
                  providerConnections[0].connection_id,
                  "reconnect",
                  "todo",
                )
              }
              className="cursor-pointer text-xs text-amber-700 underline transition-colors hover:text-amber-900"
            >
              Reconnect required
            </button>
            <span className="text-xs text-neutral-400">or</span>
            <button
              onClick={() =>
                openIntegrationUrl(
                  config.nangoIntegrationId,
                  providerConnections[0].connection_id,
                  "disconnect",
                  "todo",
                )
              }
              className="cursor-pointer text-xs text-red-500 underline transition-colors hover:text-red-700"
            >
              Disconnect
            </button>
          </div>
        ) : (
          <button
            onClick={() =>
              openIntegrationUrl(
                config.nangoIntegrationId,
                providerConnections[0].connection_id,
                "disconnect",
                "todo",
              )
            }
            className="cursor-pointer text-xs text-neutral-500 underline transition-colors hover:text-neutral-700"
          >
            Disconnect
          </button>
        )}
      </div>

      <div className="flex items-center justify-between gap-4">
        <div className="flex-1">
          <h3 className="mb-1 text-sm font-medium">{config.filterLabel}</h3>
          <p className="text-xs text-neutral-500">
            Filter synced items by {config.filterLabel.toLowerCase()}.
          </p>
        </div>
        <Input
          className="w-52"
          placeholder={config.filterPlaceholder}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
      </div>
    </div>
  );
}
