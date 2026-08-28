import { useLingui } from "@lingui/react/macro";
import {
  CaretRight,
  CircleNotch,
  DotsThree,
  Plus,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { platform } from "@tauri-apps/plugin-os";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent,
} from "react";

import type { ConnectionItem } from "@anlg/api-client";
import { colors, radii } from "@anlg/design-system/tokens.stylex";
import {
  Accordion,
  AccordionContent,
  AccordionHeader,
  AccordionItem,
  AccordionTriggerPrimitive,
} from "@anlg/ui/components/ui/accordion";

import { AppleCalendarSelection } from "./apple/calendar-selection";
import {
  AppleCalendarPermissionDialog,
  TroubleShootingLink,
} from "./apple/permission";
import { OAuthProviderContent } from "./oauth/provider-content";
import {
  type CalendarProvider,
  getCalendarConnectionKey,
  PROVIDERS,
} from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { useConnections } from "~/auth/useConnections";
import {
  allowReconnectedCalendarConnections,
  removeDisconnectedCalendarConnection,
  syncCalendarEvents,
} from "~/services/calendar";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import { usePermission } from "~/shared/hooks/usePermissions";
import { useOpenIntegrationUrl } from "~/shared/integration";

function getProviderBadgeStyle(badge: string) {
  return badge === "Beta" ? styles.betaBadge : styles.providerBadge;
}

function getDefaultOpenProviderIds(
  providers: CalendarProvider[],
  connections: ConnectionItem[] | undefined,
) {
  return providers
    .filter(
      (provider) =>
        !provider.nangoIntegrationId ||
        connections?.some(
          (connection) =>
            connection.integration_id === provider.nangoIntegrationId,
        ),
    )
    .map((provider) => provider.id);
}

function getProviderConnectionCounts(
  providers: CalendarProvider[],
  connections: ConnectionItem[] | undefined,
) {
  return new Map(
    providers
      .filter((provider) => provider.nangoIntegrationId)
      .map((provider) => [
        provider.id,
        connections?.filter(
          (connection) =>
            connection.integration_id === provider.nangoIntegrationId,
        ).length ?? 0,
      ]),
  );
}

function getProviderAccordionKey(
  providers: CalendarProvider[],
  connectionCounts: Map<string, number>,
) {
  return providers
    .map(
      (provider) => `${provider.id}:${connectionCounts.get(provider.id) ?? -1}`,
    )
    .join("|");
}

const CONNECTION_POLL_MS = 45_000;
const CONNECTION_POLL_INTERVAL_MS = 1_500;

function ProviderIcon({ provider }: { provider: CalendarProvider }) {
  return <span {...stylex.props(styles.providerIcon)}>{provider.icon}</span>;
}

export function CalendarSidebarContent({
  returnTo = "calendar",
}: {
  returnTo?: string;
}) {
  const isMacos = platform() === "macos";
  const calendar = usePermission("calendar");
  const { isPaid } = useBillingAccess();
  const [connectionPollUntil, setConnectionPollUntil] = useState<number | null>(
    null,
  );
  const connectionKeyWhenPollStartedRef = useRef("");
  const isPollingConnections = connectionPollUntil !== null;
  const { data: connections } = useConnections(isPaid, {
    refetchInterval: isPollingConnections ? CONNECTION_POLL_INTERVAL_MS : false,
  });
  const connectionKey = getCalendarConnectionKey(connections);
  const watchForNewConnection = useCallback(() => {
    connectionKeyWhenPollStartedRef.current = connectionKey;
    setConnectionPollUntil(Date.now() + CONNECTION_POLL_MS);
  }, [connectionKey]);

  useEffect(() => {
    if (connectionPollUntil === null) {
      return;
    }
    const remaining = connectionPollUntil - Date.now();
    if (remaining <= 0) {
      setConnectionPollUntil(null);
      return;
    }
    const timeoutId = window.setTimeout(() => {
      setConnectionPollUntil(null);
    }, remaining);
    return () => window.clearTimeout(timeoutId);
  }, [connectionPollUntil]);

  useEffect(() => {
    if (
      !isPollingConnections ||
      connectionKey === connectionKeyWhenPollStartedRef.current
    ) {
      return;
    }
    setConnectionPollUntil(null);
  }, [connectionKey, isPollingConnections]);

  const visibleProviders = useMemo(
    () =>
      PROVIDERS.filter(
        (p) => p.platform === "all" || (p.platform === "macos" && isMacos),
      ),
    [isMacos],
  );
  const defaultOpenProviders = useMemo(
    () => getDefaultOpenProviderIds(visibleProviders, connections),
    [connections, visibleProviders],
  );
  const providerConnectionCounts = useMemo(
    () => getProviderConnectionCounts(visibleProviders, connections),
    [connections, visibleProviders],
  );
  const accordionKey = useMemo(
    () => getProviderAccordionKey(visibleProviders, providerConnectionCounts),
    [providerConnectionCounts, visibleProviders],
  );

  return (
    <Accordion
      key={accordionKey}
      type="multiple"
      defaultValue={defaultOpenProviders}
    >
      {visibleProviders.map((provider) =>
        provider.disabled ? (
          <div key={provider.id} {...stylex.props(styles.disabledProvider)}>
            <ProviderIcon provider={provider} />
            <span {...stylex.props(styles.providerName)}>
              {provider.displayName}
            </span>
            {provider.badge && (
              <span {...stylex.props(getProviderBadgeStyle(provider.badge))}>
                {provider.badge}
              </span>
            )}
          </div>
        ) : (
          <ProviderAccordionItem
            key={provider.id}
            provider={provider}
            calendar={calendar}
            returnTo={returnTo}
            onConnectStarted={watchForNewConnection}
          />
        ),
      )}
    </Accordion>
  );
}

function ProviderAccordionItem({
  provider,
  calendar,
  returnTo,
  onConnectStarted,
}: {
  provider: CalendarProvider;
  calendar: ReturnType<typeof usePermission>;
  returnTo: string;
  onConnectStarted: () => void;
}) {
  const { t } = useLingui();
  const auth = useAuth();
  const { isPaid, isPro, upgradeToPro, isUpgradingToPro } = useBillingAccess();
  const { openIntegration, openingAction } = useOpenIntegrationUrl();
  const { data: connections, isPending, isError } = useConnections(isPaid);
  const [isApplePermissionDialogOpen, setIsApplePermissionDialogOpen] =
    useState(false);
  const providerConnections =
    connections?.filter(
      (connection) => connection.integration_id === provider.nangoIntegrationId,
    ) ?? [];

  const requiresPro = !!provider.nangoIntegrationId && !isPro;
  const appleNeedsPermission =
    provider.id === "apple" && calendar.status !== "authorized";

  const canAddAccount =
    !!provider.nangoIntegrationId &&
    !!auth.session &&
    isPaid &&
    !isPending &&
    !isError;
  const shouldConnectOnClick =
    canAddAccount && providerConnections.length === 0;

  const canDisconnectApple =
    provider.id === "apple" && calendar.status === "authorized";

  const handleAppleConnect = useCallback((): void => {
    if (calendar.isPending) return;
    allowReconnectedCalendarConnections("apple");
    if (calendar.status === "denied") {
      setIsApplePermissionDialogOpen(true);
    } else {
      calendar.request();
    }
  }, [calendar]);
  const handleAppleDisconnect = useCallback((): void => {
    void removeDisconnectedCalendarConnection("apple", "apple")
      .then(() => {
        calendar.reset();
      })
      .catch((error) => {
        console.error(
          "[calendar] failed to remove disconnected calendar data",
          error,
        );
      })
      .then(() => syncCalendarEvents())
      .catch((error) => {
        console.error("[calendar] failed to sync after disconnect", error);
      });
  }, [calendar]);
  const handleTriggerClick = useCallback(
    (event: MouseEvent<HTMLButtonElement>) => {
      if (requiresPro) {
        event.preventDefault();
        return;
      }
      if (appleNeedsPermission) {
        event.preventDefault();
        handleAppleConnect();
        return;
      }
      if (!shouldConnectOnClick) return;
      event.preventDefault();
      onConnectStarted();
      openIntegration({
        nangoIntegrationId: provider.nangoIntegrationId,
        action: "connect",
        returnTo,
      });
    },
    [
      appleNeedsPermission,
      handleAppleConnect,
      onConnectStarted,
      openIntegration,
      provider.nangoIntegrationId,
      requiresPro,
      returnTo,
      shouldConnectOnClick,
    ],
  );
  const handleAddAccount = useCallback(
    (event: MouseEvent<HTMLButtonElement>) => {
      if (!canAddAccount) return;
      event.preventDefault();
      event.stopPropagation();
      onConnectStarted();
      openIntegration({
        nangoIntegrationId: provider.nangoIntegrationId,
        action: "connect",
        returnTo,
      });
    },
    [
      canAddAccount,
      onConnectStarted,
      openIntegration,
      provider.nangoIntegrationId,
      returnTo,
    ],
  );
  const handleUpgradeToPro = useCallback(
    (event: MouseEvent<HTMLButtonElement>) => {
      event.preventDefault();
      event.stopPropagation();
      upgradeToPro();
    },
    [upgradeToPro],
  );
  const providerMenuItems = useMemo(
    (): MenuItemDef[] =>
      canAddAccount
        ? [
            {
              id: `add-${provider.id}-account`,
              text: t`Add ${provider.displayName} account`,
              action: () => {
                onConnectStarted();
                void openIntegration({
                  nangoIntegrationId: provider.nangoIntegrationId,
                  action: "connect",
                  returnTo,
                });
              },
            },
          ]
        : canDisconnectApple
          ? [
              {
                id: "reconnect-apple-calendar",
                text: t`Reconnect`,
                action: () => {
                  handleAppleConnect();
                },
                disabled: calendar.isPending,
              },
              {
                id: "disconnect-apple-calendar",
                text: t`Disconnect`,
                action: () => {
                  handleAppleDisconnect();
                },
                disabled: calendar.isPending,
              },
            ]
          : [],
    [
      calendar.isPending,
      canAddAccount,
      canDisconnectApple,
      handleAppleConnect,
      handleAppleDisconnect,
      onConnectStarted,
      provider.displayName,
      provider.id,
      provider.nangoIntegrationId,
      openIntegration,
      returnTo,
      t,
    ],
  );
  const showProviderMenu = useNativeContextMenu(providerMenuItems);
  const hasAddAccountButton = canAddAccount && !requiresPro;
  const hasProviderMenuButton = canDisconnectApple;

  return (
    <AccordionItem
      value={provider.id}
      data-provider-item
      sx={styles.accordionItem}
    >
      <div
        data-provider-row
        onContextMenu={
          providerMenuItems.length > 0 ? showProviderMenu : undefined
        }
        {...stylex.props([
          styles.providerRow,
          hasAddAccountButton || hasProviderMenuButton
            ? styles.providerRowWithActions
            : styles.providerRowWithoutActions,
        ])}
      >
        <AccordionHeader
          {...stylex.props([
            styles.accordionHeader,
            requiresPro && styles.proHeader,
          ])}
        >
          <AccordionTriggerPrimitive
            {...stylex.props(styles.accordionTrigger)}
            onClick={handleTriggerClick}
          >
            <div {...stylex.props(styles.triggerContent)}>
              <ProviderIcon provider={provider} />
              <span
                {...stylex.props([
                  styles.providerLabel,
                  requiresPro && styles.proProviderLabel,
                ])}
              >
                <span {...stylex.props(styles.providerNameTruncated)}>
                  {provider.displayName}
                </span>
                {provider.badge && (
                  <span
                    {...stylex.props(getProviderBadgeStyle(provider.badge))}
                  >
                    {provider.badge}
                  </span>
                )}
              </span>
            </div>
          </AccordionTriggerPrimitive>
        </AccordionHeader>

        {requiresPro ? (
          <button
            type="button"
            onClick={handleUpgradeToPro}
            disabled={isUpgradingToPro}
            {...stylex.props(styles.upgradeButton)}
            aria-label={t`Upgrade to Pro for ${provider.displayName}`}
          >
            {isUpgradingToPro && (
              <CircleNotch
                {...stylex.props([styles.smallIcon, styles.spinner])}
                aria-hidden="true"
              />
            )}
            {t`Upgrade to Pro`}
          </button>
        ) : appleNeedsPermission ? (
          <button
            type="button"
            onClick={handleAppleConnect}
            disabled={calendar.isPending}
            {...stylex.props(styles.circleAction)}
            aria-label={t`Connect ${provider.displayName}`}
          >
            {calendar.isPending ? (
              <CircleNotch {...stylex.props([styles.icon, styles.spinner])} />
            ) : (
              <Plus {...stylex.props(styles.icon)} />
            )}
          </button>
        ) : hasAddAccountButton ? (
          <button
            type="button"
            onClick={handleAddAccount}
            disabled={openingAction !== null}
            {...stylex.props(styles.circleAction)}
            aria-label={t`Add ${provider.displayName} account`}
          >
            {openingAction === "connect" ? (
              <CircleNotch {...stylex.props([styles.icon, styles.spinner])} />
            ) : (
              <Plus {...stylex.props(styles.icon)} />
            )}
          </button>
        ) : hasProviderMenuButton ? (
          <button
            type="button"
            onClick={showProviderMenu}
            {...stylex.props(styles.providerMenuButton)}
            aria-label={t`Open calendar account actions`}
          >
            <DotsThree {...stylex.props(styles.icon)} />
          </button>
        ) : null}

        {!requiresPro && !appleNeedsPermission && (
          <CaretRight {...stylex.props(styles.caret)} />
        )}
      </div>
      {!requiresPro && !appleNeedsPermission && (
        <AccordionContent sx={styles.accordionContent}>
          {provider.id === "apple" && (
            <div {...stylex.props(styles.appleContent)}>
              <AppleCalendarSelection
                leftAction={
                  <TroubleShootingLink
                    isPending={calendar.isPending}
                    onOpen={calendar.open}
                    onRequest={calendar.request}
                    onReset={calendar.reset}
                  />
                }
              />
            </div>
          )}
          {provider.nangoIntegrationId && (
            <OAuthProviderContent
              config={provider}
              returnTo={returnTo}
              onConnectStarted={onConnectStarted}
            />
          )}
        </AccordionContent>
      )}
      {provider.id === "apple" && (
        <AppleCalendarPermissionDialog
          open={isApplePermissionDialogOpen}
          onOpenChange={setIsApplePermissionDialogOpen}
          onOpenSettings={() => void calendar.open()}
        />
      )}
    </AccordionItem>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  accordionContent: {
    paddingBottom: "0.75rem",
  },
  accordionHeader: {
    minWidth: 0,
  },
  accordionItem: {
    borderWidth: 0,
  },
  accordionTrigger: {
    alignItems: "center",
    display: "flex",
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
    minWidth: 0,
    paddingBlock: "0.75rem",
    textAlign: "left",
    textDecorationLine: {
      default: "none",
      ":hover": "none",
    },
    transitionDuration: "150ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  appleContent: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
  },
  betaBadge: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    fontWeight: 500,
    lineHeight: "1rem",
  },
  caret: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    transform: {
      default: "rotate(0deg)",
      ":is([data-provider-item][data-state='open'] *)": "rotate(90deg)",
    },
    transitionDuration: "200ms",
    transitionProperty: "transform",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1rem",
  },
  circleAction: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.full,
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    flexShrink: 0,
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    padding: "0.25rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  disabledProvider: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    marginInline: "-0.5rem",
    opacity: 0.5,
    paddingBlock: "0.75rem",
    paddingInline: "0.5rem",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  proHeader: {
    opacity: 0.6,
  },
  proProviderLabel: {
    opacity: {
      default: 1,
      ":is([data-provider-row]:hover *)": 0,
      ":is([data-provider-row]:focus-within *)": 0,
    },
  },
  providerBadge: {
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    fontWeight: 300,
    lineHeight: "1rem",
    paddingInline: "0.5rem",
  },
  providerIcon: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    height: "1.25rem",
    justifyContent: "center",
    width: "1.25rem",
  },
  providerLabel: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    minWidth: 0,
    transitionDuration: "150ms",
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  providerMenuButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.full,
    color: colors.mutedForeground,
    flexShrink: 0,
    opacity: {
      default: 0,
      ":is([data-provider-row]:hover *)": 1,
      ":focus-visible": 1,
    },
    padding: "0.25rem",
    pointerEvents: {
      default: "none",
      ":is([data-provider-row]:hover *)": "auto",
      ":focus-visible": "auto",
    },
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  providerName: {
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
  },
  providerNameTruncated: {
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  providerRow: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.full,
    display: "grid",
    gap: "0.25rem",
    marginInline: "-0.5rem",
    paddingInline: "0.5rem",
    position: "relative",
  },
  providerRowWithActions: {
    gridTemplateColumns: "minmax(0, 1fr) auto auto",
  },
  providerRowWithoutActions: {
    gridTemplateColumns: "minmax(0, 1fr) auto",
  },
  smallIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  triggerContent: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    minWidth: 0,
  },
  upgradeButton: {
    alignItems: "center",
    backgroundColor: {
      default: colors.primary,
      ":hover": `color-mix(in oklab, ${colors.primary} 90%, transparent)`,
    },
    borderColor: colors.primary,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "2px",
    boxShadow: {
      default: "0 4px 14px rgb(87 83 78 / 0.18)",
      ":focus-visible": `0 0 0 2px ${colors.ring}, 0 4px 14px rgb(87 83 78 / 0.18)`,
    },
    color: colors.primaryForeground,
    display: "flex",
    flexShrink: 0,
    fontSize: "0.75rem",
    fontWeight: 500,
    gap: "0.25rem",
    opacity: {
      default: 0,
      ":is([data-provider-row]:hover *)": 1,
      ":is([data-provider-row]:focus-within *)": 1,
      ":disabled": 0.7,
    },
    outline: {
      default: null,
      ":focus-visible": "none",
    },
    paddingBlock: "0.25rem",
    paddingInline: "0.75rem",
    pointerEvents: {
      default: "none",
      ":is([data-provider-row]:hover *)": "auto",
      ":is([data-provider-row]:focus-within *)": "auto",
    },
    position: "absolute",
    right: "0.25rem",
    top: "50%",
    transform: {
      default: "translate(0.25rem, -50%)",
      ":is([data-provider-row]:hover *)": "translate(0, -50%)",
      ":is([data-provider-row]:focus-within *)": "translate(0, -50%)",
    },
    transitionDuration: "150ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
});
