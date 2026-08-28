import { Trans, useLingui } from "@lingui/react/macro";
import { CircleNotch } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useMemo } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@anlg/ui/components/ui/tooltip";

import { TodoFilterField, TODO_FILTER_SETTING_KEYS } from "./filter-field";
import { GitHubTodoProviderContent } from "./github";
import type { TodoProvider } from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { useConnections } from "~/auth/useConnections";
import {
  AccessPermissionRow,
  TroubleShootingLink,
} from "~/calendar/components/apple/permission";
import { usePermission } from "~/shared/hooks/usePermissions";
import { useOpenIntegrationUrl } from "~/shared/integration";

export function TodoProviderContent({ config }: { config: TodoProvider }) {
  if (config.permission === "reminders") {
    return <AppleRemindersProviderContent />;
  }

  if (config.id === "github") {
    return <GitHubTodoProviderContent config={config} />;
  }

  return <OAuthTodoProviderContent config={config} />;
}

function OAuthTodoProviderContent({ config }: { config: TodoProvider }) {
  const { t } = useLingui();

  if (!config.nangoIntegrationId) {
    return null;
  }

  const auth = useAuth();
  const { isPaid, upgradeToPro, isUpgradingToPro } = useBillingAccess();
  const { data: connections, isError } = useConnections(isPaid);
  const { openIntegration, openingAction } = useOpenIntegrationUrl();

  const providerConnections = useMemo(
    () =>
      connections?.filter(
        (connection) => connection.integration_id === config.nangoIntegrationId,
      ) ?? [],
    [connections, config.nangoIntegrationId],
  );

  const handleConnect = useCallback(
    () =>
      openIntegration({
        nangoIntegrationId: config.nangoIntegrationId,
        action: "connect",
        returnTo: "todo",
      }),
    [config.nangoIntegrationId, openIntegration],
  );

  if (!auth.session) {
    return (
      <div {...stylex.props(styles.contentPadding)}>
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <span tabIndex={0} {...stylex.props(styles.disabledConnect)}>
              <Trans>Connect {config.displayName}</Trans>
            </span>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            <Trans>Sign in to connect {config.displayName}</Trans>
          </TooltipContent>
        </Tooltip>
      </div>
    );
  }

  if (!isPaid) {
    return (
      <div {...stylex.props(styles.contentPadding)}>
        <button
          type="button"
          onClick={upgradeToPro}
          disabled={isUpgradingToPro}
          {...stylex.props(styles.linkButton)}
        >
          {isUpgradingToPro && (
            <CircleNotch {...stylex.props(styles.spinner)} aria-hidden="true" />
          )}
          <Trans>Upgrade to connect</Trans>
        </button>
      </div>
    );
  }

  if (isError) {
    return (
      <div {...stylex.props(styles.contentPadding)}>
        <span {...stylex.props(styles.error)}>
          <Trans>Failed to load integration status</Trans>
        </span>
      </div>
    );
  }

  if (providerConnections.length === 0) {
    return (
      <div {...stylex.props(styles.contentPadding)}>
        <button
          type="button"
          onClick={handleConnect}
          disabled={openingAction !== null}
          {...stylex.props(styles.linkButton)}
        >
          {openingAction === "connect" && (
            <CircleNotch {...stylex.props(styles.spinner)} aria-hidden="true" />
          )}
          <Trans>Connect {config.displayName}</Trans>
        </button>
      </div>
    );
  }

  const filterSettingKey =
    TODO_FILTER_SETTING_KEYS[
      config.id as keyof typeof TODO_FILTER_SETTING_KEYS
    ];

  return (
    <div {...stylex.props(styles.connected)}>
      <ConnectionActions
        config={config}
        providerConnections={providerConnections}
      />
      {filterSettingKey ? (
        <TodoFilterField
          settingKey={filterSettingKey}
          label={config.filterLabel ?? "Repository"}
          description={t`Only sync items from this ${(config.filterLabel ?? "repository").toLowerCase()}.`}
          placeholder={config.filterPlaceholder ?? ""}
        />
      ) : null}
    </div>
  );
}

function ConnectionActions({
  config,
  providerConnections,
}: {
  config: TodoProvider;
  providerConnections: { connection_id: string; status?: string | null }[];
}) {
  const { openIntegration, openingAction } = useOpenIntegrationUrl();

  if (!config.nangoIntegrationId || providerConnections.length === 0) {
    return null;
  }

  const reconnectRequiredConnection = providerConnections.find(
    (connection) => connection.status === "reconnect_required",
  );
  const activeConnection =
    reconnectRequiredConnection ?? providerConnections[0];

  if (reconnectRequiredConnection) {
    return (
      <div {...stylex.props(styles.actions)}>
        <button
          type="button"
          onClick={() =>
            openIntegration({
              nangoIntegrationId: config.nangoIntegrationId,
              connectionId: activeConnection.connection_id,
              action: "reconnect",
              returnTo: "todo",
            })
          }
          disabled={openingAction !== null}
          {...stylex.props(styles.linkButton, styles.reconnectButton)}
        >
          {openingAction === "reconnect" && (
            <CircleNotch {...stylex.props(styles.spinner)} aria-hidden="true" />
          )}
          <Trans>Reconnect required</Trans>
        </button>
        <span {...stylex.props(styles.or)}>
          <Trans>or</Trans>
        </span>
        <button
          type="button"
          onClick={() =>
            openIntegration({
              nangoIntegrationId: config.nangoIntegrationId,
              connectionId: activeConnection.connection_id,
              action: "disconnect",
              returnTo: "todo",
            })
          }
          disabled={openingAction !== null}
          {...stylex.props(styles.linkButton, styles.disconnectButton)}
        >
          {openingAction === "disconnect" && (
            <CircleNotch {...stylex.props(styles.spinner)} aria-hidden="true" />
          )}
          <Trans>Disconnect</Trans>
        </button>
      </div>
    );
  }

  return (
    <div {...stylex.props(styles.actions)}>
      <button
        type="button"
        onClick={() =>
          openIntegration({
            nangoIntegrationId: config.nangoIntegrationId,
            connectionId: activeConnection.connection_id,
            action: "disconnect",
            returnTo: "todo",
          })
        }
        disabled={openingAction !== null}
        {...stylex.props(styles.linkButton, styles.mutedButton)}
      >
        {openingAction === "disconnect" && (
          <CircleNotch {...stylex.props(styles.spinner)} aria-hidden="true" />
        )}
        <Trans>Disconnect</Trans>
      </button>
    </div>
  );
}

function AppleRemindersProviderContent() {
  const { t } = useLingui();
  const reminders = usePermission("reminders");

  if (reminders.status !== "authorized") {
    return (
      <AccessPermissionRow
        title={t`Reminders`}
        status={reminders.status}
        isPending={reminders.isPending}
        onOpen={reminders.open}
        onRequest={reminders.request}
        onReset={reminders.reset}
      />
    );
  }

  return (
    <TroubleShootingLink
      onRequest={reminders.request}
      onReset={reminders.reset}
      onOpen={reminders.open}
      isPending={reminders.isPending}
    />
  );
}

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  actions: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    paddingBottom: "0.25rem",
  },
  connected: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
  },
  contentPadding: {
    paddingBottom: "0.5rem",
    paddingTop: "0.25rem",
  },
  disabledConnect: {
    color: colors.mutedForeground,
    cursor: "not-allowed",
    fontSize: "0.75rem",
    lineHeight: "1rem",
    opacity: 0.5,
  },
  disconnectButton: {
    color: {
      default: "#ef4444",
      ":hover": "#b91c1c",
    },
  },
  error: {
    color: "#dc2626",
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  linkButton: {
    alignItems: "center",
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    cursor: "pointer",
    display: "inline-flex",
    fontSize: "0.75rem",
    gap: "0.25rem",
    lineHeight: "1rem",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    textDecorationLine: "underline",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  mutedButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.mutedForeground,
    },
  },
  or: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  reconnectButton: {
    color: {
      default: "#b45309",
      ":hover": "#78350f",
    },
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "0.75rem",
    width: "0.75rem",
  },
});
