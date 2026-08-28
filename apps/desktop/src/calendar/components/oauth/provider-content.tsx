import { t } from "@lingui/core/macro";
import { CircleNotch } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useMemo } from "react";

import type { ConnectionItem } from "@anlg/api-client";
import { colors } from "@anlg/design-system/tokens.stylex";
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
  const { isPro, upgradeToPro, isUpgradingToPro } = useBillingAccess();
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
      <div {...stylex.props(styles.actionSection)}>
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <span tabIndex={0} {...stylex.props(styles.disabledAction)}>
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
      <div {...stylex.props(styles.actionSection)}>
        <button
          onClick={upgradeToPro}
          disabled={isUpgradingToPro}
          {...stylex.props(styles.action)}
        >
          {isUpgradingToPro && (
            <CircleNotch {...stylex.props(styles.spinner)} aria-hidden="true" />
          )}
          {t`Upgrade to connect`}
        </button>
      </div>
    );
  }

  if (providerConnections.length > 0) {
    const reconnectRequired = providerConnections.filter(
      (c) => c.status === "reconnect_required",
    );

    return (
      <div {...stylex.props(styles.connected)}>
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
      <div {...stylex.props(styles.actionSection)}>
        <span {...stylex.props(styles.error)}>
          {t`Failed to load integration status`}
        </span>
      </div>
    );
  }

  return (
    <div {...stylex.props(styles.actionSection)}>
      <button
        onClick={handleAddAccount}
        disabled={openingAction !== null}
        {...stylex.props(styles.action)}
      >
        {openingAction === "connect" && (
          <CircleNotch {...stylex.props(styles.spinner)} aria-hidden="true" />
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
    <div {...stylex.props(styles.reconnect)}>
      <div {...stylex.props(styles.reconnectTitle)}>
        <ReconnectRequiredIndicator />
        <span>{t`Reconnect required for ${config.displayName} Calendar`}</span>
      </div>

      {errorDescription && (
        <p {...stylex.props(styles.errorDescription)}>{errorDescription}</p>
      )}

      <div {...stylex.props(styles.reconnectActions)}>
        <button
          onClick={onReconnect}
          disabled={openingAction !== null}
          {...stylex.props(styles.action)}
        >
          {openingAction === "reconnect" && (
            <CircleNotch {...stylex.props(styles.spinner)} aria-hidden="true" />
          )}
          {t`Reconnect`}
        </button>
        <span {...stylex.props(styles.or)}>{t`or`}</span>
        <button
          onClick={onDisconnect}
          disabled={openingAction !== null}
          {...stylex.props(styles.disconnectAction)}
        >
          {openingAction === "disconnect" && (
            <CircleNotch {...stylex.props(styles.spinner)} aria-hidden="true" />
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

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  action: {
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
  actionSection: {
    paddingBottom: "0.5rem",
    paddingTop: "0.25rem",
  },
  connected: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
    paddingBottom: "0.5rem",
  },
  disabledAction: {
    color: colors.mutedForeground,
    cursor: "not-allowed",
    fontSize: "0.75rem",
    lineHeight: "1rem",
    opacity: 0.5,
  },
  disconnectAction: {
    alignItems: "center",
    color: {
      default: "rgb(239 68 68)",
      ":hover": "rgb(185 28 28)",
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
  error: {
    color: "rgb(220 38 38)",
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  errorDescription: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  or: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  reconnect: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
    paddingBottom: "0.5rem",
  },
  reconnectActions: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  reconnectTitle: {
    alignItems: "center",
    color: "rgb(180 83 9)",
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.5rem",
    lineHeight: "1rem",
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
