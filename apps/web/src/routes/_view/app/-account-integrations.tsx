import { Icon } from "@iconify-icon/react";
import { Plugs, PuzzlePiece } from "@phosphor-icons/react";
import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { listConnections } from "@anlg/api-client";
import { OutlookIcon } from "@anlg/ui/components/icons/outlook";
import { cn } from "@anlg/utils";

import { getAuthorizedApiClient } from "./-account-api";
import { useAccountSession } from "./-account-session";
import {
  accountCardClassName,
  accountPillDangerClassName,
  accountPillSecondaryClassName,
} from "./-account-ui";

const INTEGRATION_NAMES: Record<string, string> = {
  "google-calendar": "Google Calendar",
  outlook: "Outlook Calendar",
  linear: "Linear",
  github: "GitHub",
  slack: "Slack",
  notion: "Notion",
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
            const hasError = !!connection.last_error_type;
            return (
              <li
                key={connection.connection_id}
                className="flex flex-col gap-4 p-6 sm:flex-row sm:items-center sm:justify-between sm:px-8"
              >
                <div className="flex items-center gap-4">
                  <span
                    aria-hidden="true"
                    className="flex size-10 shrink-0 items-center justify-center rounded-xl border border-[#ede7dc] bg-[#fffaf0]"
                  >
                    {INTEGRATION_ICONS[connection.integration_id] ?? (
                      <PuzzlePiece size={20} className="text-[#756b5d]" />
                    )}
                  </span>
                  <div>
                    <p className="text-base font-medium text-[#181613]">
                      {INTEGRATION_NAMES[connection.integration_id] ??
                        connection.integration_id}
                    </p>
                    <p className="mt-1 text-sm leading-6 text-[#756b5d]">
                      {hasError
                        ? connection.last_error_description ||
                          "Connection needs attention."
                        : "Connected."}
                    </p>
                  </div>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  {hasError && (
                    <Link
                      to="/app/integration/"
                      search={{
                        flow: "web",
                        integration_id: connection.integration_id,
                        connection_id: connection.connection_id,
                        action: "reconnect",
                      }}
                      className={accountPillSecondaryClassName}
                    >
                      Reconnect
                    </Link>
                  )}
                  <Link
                    to="/app/integration/"
                    search={{
                      flow: "web",
                      integration_id: connection.integration_id,
                      connection_id: connection.connection_id,
                      action: "disconnect",
                    }}
                    aria-label={`Disconnect ${
                      INTEGRATION_NAMES[connection.integration_id] ??
                      connection.integration_id
                    }`}
                    title="Disconnect"
                    className={cn([accountPillDangerClassName, "w-9 px-0"])}
                  >
                    <Plugs size={16} aria-hidden="true" />
                  </Link>
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
