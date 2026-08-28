import { Trans } from "@lingui/react/macro";
import { CircleNotch } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { platform } from "@tauri-apps/plugin-os";
import { motion } from "motion/react";
import { type ReactNode, useCallback, useMemo, useRef, useState } from "react";

import type { ConnectionItem } from "@anlg/api-client";
import { colors, radii } from "@anlg/design-system/tokens.stylex";

import { OnboardingButton, onboardingSharedStyles } from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { useConnections } from "~/auth/useConnections";
import { useAppleCalendarSelection } from "~/calendar/components/apple/calendar-selection";
import { TroubleShootingLink } from "~/calendar/components/apple/permission";
import {
  type CalendarGroup,
  CalendarSelection,
} from "~/calendar/components/calendar-selection";
import { SyncProvider, useSync } from "~/calendar/components/context";
import { useOAuthCalendarSelection } from "~/calendar/components/oauth/calendar-selection";
import { ReconnectRequiredIndicator } from "~/calendar/components/oauth/status";
import { PROVIDERS } from "~/calendar/components/shared";
import { useEnabledCalendars } from "~/calendar/hooks";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { usePermission } from "~/shared/hooks/usePermissions";
import { openIntegrationUrl } from "~/shared/integration";

const GOOGLE_PROVIDER = PROVIDERS.find((provider) => provider.id === "google");
const OUTLOOK_PROVIDER = PROVIDERS.find(
  (provider) => provider.id === "outlook",
);

async function openOnboardingIntegrationUrl(
  nangoIntegrationId: string | undefined,
  connectionId: string | undefined,
  action: "connect" | "reconnect" | "disconnect",
  headers: Record<string, string> | null,
) {
  await openIntegrationUrl(
    nangoIntegrationId,
    connectionId,
    action,
    undefined,
    headers,
    false,
  );
}

function getCalendarSelectionKey(groups: CalendarGroup[]) {
  return groups.length === 0
    ? "empty"
    : groups
        .map((group) => `${group.sourceName}:${group.calendars.length}`)
        .join("|");
}

function AppleCalendarList() {
  const { scheduleSync } = useSync();
  const { groups, handleRefresh, handleToggle, isLoading } =
    useAppleCalendarSelection();

  useMountEffect(() => {
    scheduleSync();
  });

  return (
    <CalendarSelection
      key={getCalendarSelectionKey(groups)}
      groups={groups}
      onToggle={handleToggle}
      onRefresh={handleRefresh}
      isLoading={isLoading}
      disableHoverTone
      sx={styles.calendarSelection}
    />
  );
}

function AppleCalendarProvider({
  isAuthorized,
  isPending,
  onRequest,
  onTroubleshoot,
  onOpen,
}: {
  isAuthorized: boolean;
  isPending: boolean;
  onRequest: () => void;
  onTroubleshoot: () => void;
  onOpen: () => void;
}) {
  return (
    <>
      {isAuthorized && (
        <div {...stylex.props(styles.connectedCalendar)}>
          <AppleCalendarList />
        </div>
      )}

      <div {...stylex.props(styles.providerActionContainer)}>
        <OnboardingButton
          onClick={() => {
            if (isAuthorized) {
              onOpen();
              return;
            }

            onTroubleshoot();
            onRequest();
          }}
          disabled={isPending}
          sx={styles.connectedProviderButton}
        >
          <img
            src="/assets/apple-calendar.png"
            alt=""
            aria-hidden="true"
            {...stylex.props(styles.providerIcon)}
          />
          <Trans>Connect calendar</Trans>
        </OnboardingButton>
      </div>
    </>
  );
}

function GoogleCalendarConnectedContent({
  providerConnections,
}: {
  providerConnections: ConnectionItem[];
}) {
  const auth = useAuth();
  const { scheduleSync } = useSync();
  const {
    groups,
    connectionSourceMap,
    handleRefresh,
    handleToggle,
    isLoading,
  } = useOAuthCalendarSelection(GOOGLE_PROVIDER!);
  const reconnectRequiredConnections = useMemo(
    () =>
      providerConnections.filter(
        (connection) => connection.status === "reconnect_required",
      ),
    [providerConnections],
  );
  const groupsWithMenus = useMemo(
    () =>
      addIntegrationMenus({
        groups,
        connections: providerConnections,
        connectionSourceMap,
        provider: GOOGLE_PROVIDER!,
        getHeaders: auth.getHeaders,
      }),
    [auth.getHeaders, connectionSourceMap, groups, providerConnections],
  );

  useMountEffect(() => {
    scheduleSync();
  });

  return (
    <div {...stylex.props(styles.connectedContent)}>
      {reconnectRequiredConnections.length > 0 && (
        <div {...stylex.props(styles.warning)}>
          <span {...stylex.props(styles.warningIndicator)}>
            <ReconnectRequiredIndicator />
          </span>
          <p>
            Some Google Calendar accounts need attention. Open the account menu
            to reconnect or disconnect them.
          </p>
        </div>
      )}

      <CalendarSelection
        key={getCalendarSelectionKey(groupsWithMenus)}
        groups={groupsWithMenus}
        onToggle={handleToggle}
        onRefresh={handleRefresh}
        isLoading={isLoading}
        disableHoverTone
        sx={styles.calendarSelection}
      />
    </div>
  );
}

function addIntegrationMenus({
  groups,
  connections,
  connectionSourceMap,
  provider,
  getHeaders,
}: {
  groups: CalendarGroup[];
  connections: ConnectionItem[];
  connectionSourceMap: Map<string, string>;
  provider: (typeof PROVIDERS)[number];
  getHeaders: () => Record<string, string> | null;
}) {
  return groups.map((group) => {
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
          text: "Reconnect",
          action: () =>
            void openOnboardingIntegrationUrl(
              provider.nangoIntegrationId,
              connection.connection_id,
              "reconnect",
              getHeaders(),
            ),
        },
        {
          id: `disconnect-${connection.connection_id}`,
          text: "Disconnect",
          action: () =>
            void openOnboardingIntegrationUrl(
              provider.nangoIntegrationId,
              connection.connection_id,
              "disconnect",
              getHeaders(),
            ),
        },
      ],
    };
  });
}

function OutlookCalendarConnectedContent({
  providerConnections,
}: {
  providerConnections: ConnectionItem[];
}) {
  const auth = useAuth();
  const { scheduleSync } = useSync();
  const {
    groups,
    connectionSourceMap,
    handleRefresh,
    handleToggle,
    isLoading,
  } = useOAuthCalendarSelection(OUTLOOK_PROVIDER!);
  const reconnectRequiredConnections = useMemo(
    () =>
      providerConnections.filter(
        (connection) => connection.status === "reconnect_required",
      ),
    [providerConnections],
  );
  const groupsWithMenus = useMemo(
    () =>
      addIntegrationMenus({
        groups,
        connections: providerConnections,
        connectionSourceMap,
        provider: OUTLOOK_PROVIDER!,
        getHeaders: auth.getHeaders,
      }),
    [auth.getHeaders, connectionSourceMap, groups, providerConnections],
  );

  useMountEffect(() => {
    scheduleSync();
  });

  return (
    <div {...stylex.props(styles.connectedContent)}>
      {reconnectRequiredConnections.length > 0 && (
        <div {...stylex.props(styles.warning)}>
          <span {...stylex.props(styles.warningIndicator)}>
            <ReconnectRequiredIndicator />
          </span>
          <p>
            Some Outlook accounts need attention. Open the account menu to
            reconnect or disconnect them.
          </p>
        </div>
      )}

      <CalendarSelection
        key={getCalendarSelectionKey(groupsWithMenus)}
        groups={groupsWithMenus}
        onToggle={handleToggle}
        onRefresh={handleRefresh}
        isLoading={isLoading}
        disableHoverTone
        sx={styles.calendarSelection}
      />
    </div>
  );
}

function OAuthCalendarProviderAction({
  provider,
  connectLabel,
  isConnected,
  isHovered,
  isOpening,
  isPending,
  isReady,
  isSignedIn,
  onConnect,
  onHoverChange,
}: {
  provider: (typeof PROVIDERS)[number];
  connectLabel: ReactNode;
  isConnected: boolean;
  isHovered: boolean;
  isOpening: boolean;
  isPending: boolean;
  isReady: boolean;
  isSignedIn: boolean;
  onConnect: () => void;
  onHoverChange: (hovered: boolean) => void;
}) {
  return (
    <div {...stylex.props(styles.providerActionContainer)}>
      <OnboardingButton
        onClick={onConnect}
        onMouseEnter={() => onHoverChange(true)}
        onMouseLeave={() => onHoverChange(false)}
        onFocus={() => onHoverChange(true)}
        onBlur={() => onHoverChange(false)}
        disabled={isOpening || (isSignedIn && (isPending || !isReady))}
        sx={[
          styles.providerButton,
          isSignedIn
            ? styles.signedInProviderButton
            : styles.signedOutProviderButton,
        ]}
      >
        {!isSignedIn ? (
          <span {...stylex.props(styles.slidingLabel)}>
            <span
              {...stylex.props([styles.slidingLabelItem, styles.measureLabel])}
            >
              <Trans>Sign in to connect</Trans>
            </span>

            <motion.span
              {...stylex.props(styles.slidingLabelItem)}
              animate={{ y: isHovered ? "100%" : "0%" }}
              transition={{ type: "spring", bounce: 0.15, duration: 0.35 }}
            >
              {provider.icon}
              <span {...stylex.props(styles.providerName)}>
                {provider.displayName}
              </span>
            </motion.span>

            <motion.span
              {...stylex.props(styles.slidingLabelItem)}
              animate={{ y: isHovered ? "0%" : "-140%" }}
              transition={{ type: "spring", bounce: 0.15, duration: 0.35 }}
            >
              <Trans>Sign in to connect</Trans>
            </motion.span>
          </span>
        ) : (
          <>
            {isOpening ? (
              <CircleNotch
                {...stylex.props([
                  styles.loadingIcon,
                  onboardingSharedStyles.spin,
                ])}
                aria-hidden="true"
              />
            ) : (
              provider.icon
            )}
            {isConnected ? <Trans>Add another account</Trans> : connectLabel}
          </>
        )}
      </OnboardingButton>
    </div>
  );
}

function OutlookCalendarProvider({ onSignIn }: { onSignIn: () => void }) {
  const auth = useAuth();
  const { isPro, isReady, upgradeToPro, isUpgradingToPro } = useBillingAccess();
  const { data: connections, isPending, isError } = useConnections(isPro);
  const [isHovered, setHovered] = useState(false);
  const [isOpening, setIsOpening] = useState(false);
  // State alone cannot gate re-entry: a second click can land before the
  // disabled state commits and open a duplicate connect flow.
  const openInFlightRef = useRef(false);
  const providerConnections = useMemo(
    () =>
      connections?.filter(
        (connection) =>
          connection.integration_id === OUTLOOK_PROVIDER?.nangoIntegrationId,
      ) ?? [],
    [connections],
  );

  const handleConnect = useCallback(() => {
    if (!auth.session) {
      onSignIn();
      return;
    }

    if (!isPro) {
      upgradeToPro();
      return;
    }

    if (openInFlightRef.current) {
      return;
    }
    openInFlightRef.current = true;
    setIsOpening(true);
    void openOnboardingIntegrationUrl(
      OUTLOOK_PROVIDER?.nangoIntegrationId,
      undefined,
      "connect",
      auth.getHeaders(),
    ).finally(() => {
      openInFlightRef.current = false;
      setIsOpening(false);
    });
  }, [auth.getHeaders, auth.session, isPro, onSignIn, upgradeToPro]);

  if (!OUTLOOK_PROVIDER) {
    return null;
  }

  if (isError) {
    return (
      <p {...stylex.props(styles.providerError)}>
        <Trans>Failed to load Outlook Calendar</Trans>
      </p>
    );
  }

  const isSignedIn = !!auth.session;
  const isConnected = providerConnections.length > 0;

  return (
    <>
      {isConnected && (
        <div {...stylex.props(styles.connectedCalendar)}>
          <OutlookCalendarConnectedContent
            providerConnections={providerConnections}
          />
        </div>
      )}

      <OAuthCalendarProviderAction
        provider={OUTLOOK_PROVIDER}
        connectLabel={<Trans>Connect Outlook</Trans>}
        isConnected={isConnected}
        isHovered={isHovered}
        isOpening={isOpening || isUpgradingToPro}
        isPending={isPending}
        isReady={isReady}
        isSignedIn={isSignedIn}
        onConnect={handleConnect}
        onHoverChange={setHovered}
      />
    </>
  );
}

function GoogleCalendarProvider({ onSignIn }: { onSignIn: () => void }) {
  const auth = useAuth();
  const { isPro, isReady, upgradeToPro, isUpgradingToPro } = useBillingAccess();
  const { data: connections, isPending, isError } = useConnections(isPro);
  const [isHovered, setHovered] = useState(false);
  const [isOpening, setIsOpening] = useState(false);
  // State alone cannot gate re-entry: a second click can land before the
  // disabled state commits and open a duplicate connect flow.
  const openInFlightRef = useRef(false);
  const providerConnections = useMemo(
    () =>
      connections?.filter(
        (connection) =>
          connection.integration_id === GOOGLE_PROVIDER?.nangoIntegrationId,
      ) ?? [],
    [connections],
  );

  const handleConnect = useCallback(() => {
    if (!auth.session) {
      onSignIn();
      return;
    }

    if (!isPro) {
      upgradeToPro();
      return;
    }

    if (openInFlightRef.current) {
      return;
    }
    openInFlightRef.current = true;
    setIsOpening(true);
    void openOnboardingIntegrationUrl(
      GOOGLE_PROVIDER?.nangoIntegrationId,
      undefined,
      "connect",
      auth.getHeaders(),
    ).finally(() => {
      openInFlightRef.current = false;
      setIsOpening(false);
    });
  }, [auth.getHeaders, auth.session, isPro, onSignIn, upgradeToPro]);

  if (!GOOGLE_PROVIDER) {
    return null;
  }

  if (isError) {
    return (
      <p {...stylex.props(styles.providerError)}>
        <Trans>Failed to load Google Calendar</Trans>
      </p>
    );
  }

  const isSignedIn = !!auth.session;
  const isConnected = providerConnections.length > 0;

  return (
    <>
      {isConnected && (
        <div {...stylex.props(styles.connectedCalendar)}>
          <GoogleCalendarConnectedContent
            providerConnections={providerConnections}
          />
        </div>
      )}

      <OAuthCalendarProviderAction
        provider={GOOGLE_PROVIDER}
        connectLabel={<Trans>Connect Google Calendar</Trans>}
        isConnected={isConnected}
        isHovered={isHovered}
        isOpening={isOpening || isUpgradingToPro}
        isPending={isPending}
        isReady={isReady}
        isSignedIn={isSignedIn}
        onConnect={handleConnect}
        onHoverChange={setHovered}
      />
    </>
  );
}

function CalendarSectionContent({
  onContinue,
  onSignIn,
}: {
  onContinue: () => void;
  onSignIn: () => void;
}) {
  const isMacos = platform() === "macos";
  const calendar = usePermission("calendar");
  const isAuthorized = calendar.status === "authorized";
  const [showTroubleshooting, setShowTroubleshooting] = useState(false);
  const enabledCalendars = useEnabledCalendars();
  const hasConnectedCalendar = enabledCalendars.length > 0;

  return (
    <div {...stylex.props(styles.section)}>
      <div {...stylex.props(styles.providers)}>
        {isMacos && (
          <AppleCalendarProvider
            isAuthorized={isAuthorized}
            isPending={calendar.isPending}
            onRequest={calendar.request}
            onTroubleshoot={() => setShowTroubleshooting(true)}
            onOpen={calendar.open}
          />
        )}

        <GoogleCalendarProvider onSignIn={onSignIn} />
        <OutlookCalendarProvider onSignIn={onSignIn} />
      </div>

      {hasConnectedCalendar && (
        <OnboardingButton onClick={onContinue}>
          <Trans>Continue</Trans>
        </OnboardingButton>
      )}

      {showTroubleshooting && !isAuthorized && (
        <TroubleShootingLink
          onRequest={calendar.request}
          onReset={calendar.reset}
          onOpen={calendar.open}
          isPending={calendar.isPending}
          sx={styles.troubleshooting}
        />
      )}
    </div>
  );
}

export function CalendarSection({
  onContinue,
  onSignIn,
}: {
  onContinue: () => void;
  onSignIn: () => void;
}) {
  return (
    <SyncProvider>
      <CalendarSectionContent onContinue={onContinue} onSignIn={onSignIn} />
    </SyncProvider>
  );
}

const styles = stylex.create({
  calendarSelection: {
    backdropFilter: "blur(12px) saturate(1.5)",
    backgroundColor: `color-mix(in srgb, ${colors.card} 28%, transparent)`,
    borderColor: `color-mix(in srgb, ${colors.border} 45%, transparent)`,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow:
      "inset 0 1px 0 rgb(255 255 255 / 0.4), 0 8px 24px -20px rgb(87 83 78 / 0.35)",
    padding: "1rem",
  },
  connectedCalendar: {
    flexBasis: "100%",
    order: 1,
    width: "100%",
  },
  connectedContent: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
  },
  connectedProviderButton: {
    alignItems: "center",
    backgroundColor: {
      default: colors.card,
      ":hover": colors.accent,
    },
    borderColor: colors.border,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow:
      "0 2px 6px rgb(87 83 78 / 0.08), 0 10px 18px -10px rgb(87 83 78 / 0.22)",
    color: colors.foreground,
    display: "flex",
    gap: "0.75rem",
    height: "100%",
    justifyContent: "center",
    paddingInline: "1.5rem",
    transitionDuration: "150ms",
    transitionProperty: "all",
    width: "100%",
  },
  loadingIcon: {
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  measureLabel: {
    visibility: "hidden",
  },
  providerActionContainer: {
    display: "flex",
    flex: "1",
    minWidth: "14rem",
    order: 2,
  },
  providerButton: {
    alignItems: "center",
    borderColor: colors.border,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    gap: "0.75rem",
    height: "100%",
    justifyContent: "center",
    width: "100%",
  },
  providerError: {
    color: "rgb(220 38 38)",
    flex: "1",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    minWidth: "14rem",
    order: 2,
  },
  providerIcon: {
    borderRadius: "4px",
    height: "1.5rem",
    objectFit: "cover",
    width: "1.5rem",
  },
  providerName: {
    color: colors.foreground,
    fontSize: "1rem",
    fontWeight: 400,
    lineHeight: "1.5rem",
  },
  providers: {
    alignItems: "stretch",
    display: "flex",
    flexWrap: "wrap",
    gap: "0.75rem",
  },
  section: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  signedInProviderButton: {
    backgroundColor: {
      default: colors.card,
      ":hover": colors.accent,
      ":disabled:hover": colors.card,
    },
    boxShadow:
      "0 2px 6px rgb(87 83 78 / 0.08), 0 10px 18px -10px rgb(87 83 78 / 0.22)",
    color: colors.foreground,
    cursor: {
      default: "pointer",
      ":disabled": "not-allowed",
    },
    opacity: {
      default: 1,
      ":disabled": 0.6,
    },
  },
  signedOutProviderButton: {
    backgroundColor: {
      default: colors.muted,
      ":hover": colors.primary,
      ":focus-visible": colors.primary,
    },
    borderColor: {
      default: colors.border,
      ":hover": colors.primary,
      ":focus-visible": colors.primary,
    },
    boxShadow:
      "0 2px 6px rgb(87 83 78 / 0.01), 0 10px 18px -10px rgb(87 83 78 / 0.1)",
    color: {
      default: colors.foreground,
      ":hover": colors.primaryForeground,
      ":focus-visible": colors.primaryForeground,
    },
    transitionDuration: "150ms",
    transitionProperty: "all",
  },
  slidingLabel: {
    alignItems: "center",
    display: "grid",
    overflow: "hidden",
  },
  slidingLabelItem: {
    alignItems: "center",
    display: "flex",
    gap: "0.75rem",
    gridColumnStart: "1",
    gridRowStart: "1",
    justifyContent: "center",
  },
  troubleshooting: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  warning: {
    alignItems: "flex-start",
    color: "rgb(180 83 9)",
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    lineHeight: "1.25rem",
  },
  warningIndicator: {
    paddingTop: "0.25rem",
  },
});
