import { Link, useNavigate } from "@tanstack/react-router";
import { ChevronDown } from "lucide-react";

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";

import { useBilling } from "@/hooks/use-billing";
import { useConnections } from "@/hooks/use-connections";

const INTEGRATIONS = [
  { id: "google-calendar", name: "Google Calendar" },
] as const;

export function IntegrationsSettingsCard() {
  const navigate = useNavigate();
  const { isPro } = useBilling();
  const { data: connections, isLoading } = useConnections(isPro);

  const getConnectionStatus = (integrationId: string) => {
    return connections?.find((c) => c.integration_id === integrationId);
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
        const connection = getConnectionStatus(integration.id);
        const isConnected = !!connection;

        return (
          <div
            key={integration.id}
            className="flex items-center justify-between border-t border-neutral-100 p-4"
          >
            <div className="flex items-center gap-3">
              <div className="text-sm font-medium">{integration.name}</div>
            </div>

            {!isPro ? (
              <Link
                to="/app/checkout/"
                search={{ period: "monthly" }}
                className="flex h-8 items-center rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-4 text-sm text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%]"
              >
                Upgrade to Pro
              </Link>
            ) : isLoading ? (
              <button
                disabled
                className="flex h-8 items-center rounded-full border border-neutral-300 bg-linear-to-b from-white to-stone-50 px-4 text-sm text-neutral-500 shadow-xs"
              >
                Loading...
              </button>
            ) : isConnected && connection ? (
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <button className="flex h-8 cursor-pointer items-center gap-1 rounded-full border border-neutral-300 bg-linear-to-b from-white to-stone-50 px-4 text-sm text-neutral-700 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]">
                    Connected
                    <ChevronDown size={14} />
                  </button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="w-40">
                  <DropdownMenuItem
                    onClick={() =>
                      navigate({
                        to: "/app/integration/",
                        search: {
                          flow: "web",
                          integration_id: integration.id,
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
                          integration_id: integration.id,
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
                className="flex h-8 cursor-pointer items-center rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-4 text-sm text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%]"
              >
                Connect
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}
