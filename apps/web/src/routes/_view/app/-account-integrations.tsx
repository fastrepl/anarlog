import { Link, useNavigate } from "@tanstack/react-router";
import { ChevronDown, PlusIcon } from "lucide-react";

import type { ConnectionItem, WhoAmIItem } from "@hypr/api-client";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";

import { useBilling } from "@/hooks/use-billing";
import { useConnections } from "@/hooks/use-connections";
import { useWhoAmI } from "@/hooks/use-whoami";

const INTEGRATIONS = [
  { id: "google-calendar", name: "Google Calendar" },
] as const;

export function IntegrationsSettingsCard() {
  const navigate = useNavigate();
  const { isPaid } = useBilling();
  const { data: connections, isLoading } = useConnections(isPaid);
  const { data: accounts } = useWhoAmI(isPaid);

  const getProviderConnections = (integrationId: string) => {
    return connections?.filter((c) => c.integration_id === integrationId) ?? [];
  };

  const getAccountInfo = (connectionId: string) => {
    return accounts?.find((a) => a.connection_id === connectionId);
  };

  return (
    <div className="rounded-xs border border-neutral-100">
      <div className="p-4">
        <h3 className="mb-2 font-serif text-lg font-semibold">Integrations</h3>
        <p className="text-sm text-neutral-600">
          Connect third-party services to enhance your experience
        </p>
      </div>

      {INTEGRATIONS.map((integration) => {
        const providerConnections = getProviderConnections(integration.id);

        return (
          <div key={integration.id} className="border-t border-neutral-100 p-4">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="text-sm font-medium">{integration.name}</div>
              </div>

              {!isPaid ? (
                <Link
                  to="/pricing/"
                  className="flex h-8 items-center rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-4 text-sm text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%]"
                >
                  Upgrade
                </Link>
              ) : isLoading ? (
                <button
                  disabled
                  className="flex h-8 items-center rounded-full border border-neutral-300 bg-linear-to-b from-white to-stone-50 px-4 text-sm text-neutral-500 shadow-xs"
                >
                  Loading...
                </button>
              ) : (
                <button
                  onClick={() =>
                    navigate({
                      to: "/app/integration/",
                      search: {
                        flow: "web",
                        integration_id: integration.id,
                        action: "connect",
                      },
                    })
                  }
                  className="flex h-8 cursor-pointer items-center gap-1 rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-4 text-sm text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%]"
                >
                  <PlusIcon size={14} />
                  {providerConnections.length > 0 ? "Add account" : "Connect"}
                </button>
              )}
            </div>

            {providerConnections.length > 0 && (
              <div className="mt-3 flex flex-col gap-2">
                {providerConnections.map((connection) => (
                  <ConnectionRow
                    key={connection.connection_id}
                    connection={connection}
                    integrationId={integration.id}
                    account={getAccountInfo(connection.connection_id)}
                  />
                ))}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}

function ConnectionRow({
  connection,
  integrationId,
  account,
}: {
  connection: ConnectionItem;
  integrationId: string;
  account?: WhoAmIItem;
}) {
  const navigate = useNavigate();
  const isReconnectRequired = connection.status === "reconnect_required";
  const displayLabel =
    account?.email ?? account?.display_name ?? connection.connection_id;

  return (
    <div className="flex items-center justify-between rounded-md border border-neutral-100 px-3 py-2">
      <div className="flex items-center gap-2">
        <span
          className={[
            "size-2 rounded-full",
            isReconnectRequired ? "bg-amber-500" : "bg-green-500",
          ].join(" ")}
        />
        <span className="text-xs text-neutral-700">{displayLabel}</span>
        {isReconnectRequired && (
          <span className="text-xs text-amber-600">Reconnect required</span>
        )}
      </div>

      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button className="flex h-7 cursor-pointer items-center gap-1 rounded-full border border-neutral-200 bg-linear-to-b from-white to-stone-50 px-3 text-xs text-neutral-600 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]">
            {isReconnectRequired ? "Reconnect" : "Manage"}
            <ChevronDown size={12} />
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-40">
          <DropdownMenuItem
            onClick={() =>
              navigate({
                to: "/app/integration/",
                search: {
                  flow: "web",
                  integration_id: integrationId,
                  action: "connect",
                  connection_id: connection.connection_id,
                },
              })
            }
          >
            Reconnect
          </DropdownMenuItem>
          <DropdownMenuItem
            onClick={() =>
              navigate({
                to: "/app/integration/",
                search: {
                  flow: "web",
                  action: "disconnect",
                  integration_id: integrationId,
                  connection_id: connection.connection_id,
                },
              })
            }
            className="text-red-600 focus:text-red-600"
          >
            Disconnect
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
