import { Icon } from "@iconify-icon/react";
import { DotsThree, PuzzlePiece } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { listConnections } from "@anlg/api-client";
import { OutlookIcon } from "@anlg/ui/components/icons/outlook";
import {
  AppFloatingPanel,
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
import { accountStyles } from "./-account-ui";
const styles = stylex.create({
  style1: {
    padding: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2rem",
    },
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style2: {
    borderBottomColor: {
      ":is(*) > :not(:last-child)": "#ede7dc",
    },
    borderBottomStyle: {
      ":is(*) > :not(:last-child)": "solid",
    },
    borderBottomWidth: {
      ":is(*) > :not(:last-child)": "1px",
    },
  },
  style3: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: ".75rem",
    padding: "1.5rem",
    paddingInline: {
      default: null,
      "@media (width >= 40rem)": "2rem",
    },
  },
  style4: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    gap: "1rem",
  },
  style5: {
    display: "flex",
    width: "2.5rem",
    height: "2.5rem",
    flexShrink: 0,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: ".75rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "#ede7dc",
    backgroundColor: "#fffaf0",
  },
  style6: {
    color: "#756b5d",
  },
  style7: {
    minWidth: 0,
  },
  style8: {
    fontSize: "1rem",
    lineHeight: "1.5rem",
    fontWeight: 500,
    color: "#181613",
  },
  style9: {
    marginTop: ".25rem",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    overflow: "hidden",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style10: {
    width: "11rem",
  },
  style11: {
    overflow: "hidden",
    padding: ".25rem",
  },
  style12: {
    cursor: "pointer",
  },
  style13: {
    cursor: "pointer",
    color: {
      default: "#b91c1c",
      ":focus": "#991b1b",
    },
    backgroundColor: {
      default: null,
      ":focus": "#fef2f2",
    },
  },
});
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
      const { data, error } = await listConnections({
        client,
      });
      if (error || !data) {
        throw new Error("Failed to load connections");
      }
      return data.connections;
    },
  });
  const connections = connectionsQuery.data ?? [];
  return (
    <div {...stylex.props(accountStyles.card)}>
      {connectionsQuery.isPending || session.isPending ? (
        <p {...stylex.props(styles.style1)}>Checking your connections...</p>
      ) : connectionsQuery.isError ? (
        <p {...stylex.props(styles.style1)}>
          We could not load your connections. Refresh the page to try again.
        </p>
      ) : connections.length === 0 ? (
        <p {...stylex.props(styles.style1)}>
          {/* The API gates /integration on any paid entitlement, so Lite users
              already have access and must not see the upsell. */}
          {session.data?.billing.isPaid
            ? "Nothing connected yet. Connect calendars and tools from the desktop app."
            : "Integrations come with a paid plan and connect from the desktop app."}
        </p>
      ) : (
        <ul {...stylex.props(styles.style2)}>
          {connections.map((connection) => {
            const name =
              INTEGRATION_NAMES[connection.integration_id] ??
              connection.integration_id;
            const needsReconnect = connectionNeedsReconnect(connection);
            const reconnectError = connectionReconnectError(connection);
            return (
              <li
                key={connection.connection_id}
                {...stylex.props(styles.style3)}
              >
                <div {...stylex.props(styles.style4)}>
                  <span aria-hidden="true" {...stylex.props(styles.style5)}>
                    {INTEGRATION_ICONS[connection.integration_id] ?? (
                      <PuzzlePiece size={20} {...stylex.props(styles.style6)} />
                    )}
                  </span>
                  <div {...stylex.props(styles.style7)}>
                    <p {...stylex.props(styles.style8)}>{name}</p>
                    <p {...stylex.props(styles.style9)}>
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
          {...stylex.props(accountStyles.menuTrigger)}
        >
          <DotsThree size={16} aria-hidden="true" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent variant="app" align="end" sx={styles.style10}>
        <AppFloatingPanel sx={styles.style11}>
          {needsReconnect && (
            <>
              <DropdownMenuItem asChild sx={styles.style12}>
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
          <DropdownMenuItem asChild sx={styles.style13}>
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
