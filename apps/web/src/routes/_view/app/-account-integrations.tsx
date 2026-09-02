import { Icon } from "@iconify-icon/react";
import { DotsThree, PuzzlePiece } from "@phosphor-icons/react";
import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { listConnections } from "@anlg/api-client";
import { OutlookIcon } from "@anlg/ui/components/icons/outlook";
import {
  AppFloatingPanel,
  appFloatingMenuPanelClassName,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";

import {
  connectionIdentityLabel,
  connectionNeedsReconnect,
  connectionReconnectError,
} from "@/lib/integration-connection-label";

import { getAuthorizedApiClient } from "./-account-api";
import { useAccountSession } from "./-account-session";
import {
  accountCardClassName,
  accountMenuTriggerClassName,
} from "./-account-ui";

const INTEGRATION_NAMES: Record<string, string> = {
  "google-calendar": "Google Calendar",
  outlook: "Outlook Calendar",
  linear: "Linear",
  github: "GitHub",
  slack: "Slack",
  notion: "Notion",
  zoom: "Zoom",
  fathom: "Fathom",
  webex: "Webex",
  "google-meet": "Google Meet",
  "microsoft-teams": "Microsoft Teams",
};

const INTEGRATION_ICONS: Record<string, ReactNode> = {
  "google-calendar": (
    <Icon icon="logos:google-calendar" width="20" height="20" />
  ),
  outlook: <OutlookIcon size={20} />,
  linear: <Icon icon="logos:linear-icon" width="20" height="20" />,
  github: <Icon icon="logos:github-icon" width="20" height="20" />,
  slack: <Icon icon="logos:slack-icon" width="20" height="20" />,
  notion: <Icon icon="logos:notion-icon" width="20" height="20" />,
  zoom: <Icon icon="logos:zoom-icon" width="20" height="20" />,
  fathom: <Icon icon="simple-icons:fathom" width="20" height="20" />,
  webex: <Icon icon="simple-icons:cisco" width="20" height="20" />,
  "google-meet": <Icon icon="logos:google-meet" width="20" height="20" />,
  "microsoft-teams": (
    <Icon icon="logos:microsoft-teams" width="20" height="20" />
  ),
};

const connectionsQueryKey = ["account-integrations"];

export function IntegrationsSection() {
  const session = useAccountSession();

  const connectionsQuery = useQuery({
    queryKey: connectionsQueryKey,
    // Skip the SSR fetch: the browser-only access token throws on the
    // server, and this data is session-scoped anyway.
    enabled: typeof window !== "undefined",
    queryFn: async () => {
      const client = await getAuthorizedApiClient();
      const { data, error } = await listConnections({ client });
      if (error || !data) {
        throw new Error("Failed to load connections");
      }
      return data.connections;
    },
  });

  const connections = connectionsQuery.data ?? [];

  return (
    <div className={accountCardClassName}>
      {connectionsQuery.isPending || session.isPending ? (
        <p className="p-6 text-sm leading-6 text-[#756b5d] sm:p-8">
          Checking your connections...
        </p>
      ) : connectionsQuery.isError ? (
        <p className="p-6 text-sm leading-6 text-[#756b5d] sm:p-8">
          We could not load your connections. Refresh the page to try again.
        </p>
      ) : connections.length === 0 ? (
        <p className="p-6 text-sm leading-6 text-[#756b5d] sm:p-8">
          {/* The API gates /integration on any paid entitlement, so Lite users
              already have access and must not see the upsell. */}
          {session.data?.billing.isPaid
            ? "Nothing connected yet. Connect calendars and tools from the desktop app."
            : "Integrations come with a paid plan and connect from the desktop app."}
        </p>
      ) : (
        <ul className="divide-y divide-[#ede7dc]">
          {connections.map((connection) => {
            const name =
              INTEGRATION_NAMES[connection.integration_id] ??
              connection.integration_id;
            const needsReconnect = connectionNeedsReconnect(connection);
            const reconnectError = connectionReconnectError(connection);

            return (
              <li
                key={connection.connection_id}
                className="flex items-center justify-between gap-3 p-6 sm:px-8"
              >
                <div className="flex min-w-0 items-center gap-4">
                  <span
                    aria-hidden="true"
                    className="flex size-10 shrink-0 items-center justify-center rounded-xl border border-[#ede7dc] bg-[#fffaf0]"
                  >
                    {INTEGRATION_ICONS[connection.integration_id] ?? (
                      <PuzzlePiece size={20} className="text-[#756b5d]" />
                    )}
                  </span>
                  <div className="min-w-0">
                    <p className="text-base font-medium text-[#181613]">
                      {name}
                    </p>
                    <p className="mt-1 truncate text-sm leading-6 text-[#756b5d]">
                      {connectionIdentityLabel(connection)}
                    </p>
                  </div>
                </div>
                <IntegrationRowMenu
                  name={name}
                  integrationId={connection.integration_id}
                  connectionId={connection.connection_id}
                  needsReconnect={needsReconnect}
                  reconnectError={reconnectError}
                />
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

function IntegrationRowMenu({
  name,
  integrationId,
  connectionId,
  needsReconnect,
  reconnectError,
}: {
  name: string;
  integrationId: string;
  connectionId: string;
  needsReconnect: boolean;
  reconnectError: string;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={`Actions for ${name}`}
          className={accountMenuTriggerClassName}
        >
          <DotsThree size={16} aria-hidden="true" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent variant="app" align="end" className="w-44">
        <AppFloatingPanel className={appFloatingMenuPanelClassName}>
          {needsReconnect && (
            <>
              <DropdownMenuItem asChild className="cursor-pointer">
                <Link
                  to="/app/integration/"
                  search={{
                    flow: "web",
                    integration_id: integrationId,
                    connection_id: connectionId,
                    action: "reconnect",
                  }}
                  aria-label={
                    reconnectError
                      ? `Reconnect ${name}. ${reconnectError}`
                      : `Reconnect ${name}`
                  }
                  title={reconnectError}
                >
                  Reconnect
                </Link>
              </DropdownMenuItem>
              <DropdownMenuSeparator />
            </>
          )}
          <DropdownMenuItem
            asChild
            className="cursor-pointer text-red-700 focus:bg-red-50 focus:text-red-800"
          >
            <Link
              to="/app/integration/"
              search={{
                flow: "web",
                integration_id: integrationId,
                connection_id: connectionId,
                action: "disconnect",
              }}
              aria-label={`Disconnect ${name}`}
            >
              Disconnect
            </Link>
          </DropdownMenuItem>
        </AppFloatingPanel>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
