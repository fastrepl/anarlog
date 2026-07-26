import { Trans } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import { Loader2Icon } from "lucide-react";
import { motion } from "motion/react";
import { type ReactNode, useCallback, useMemo, useRef, useState } from "react";

import type { ConnectionItem } from "@hypr/api-client";
import { commands as openerCommands } from "@hypr/plugin-opener2";

import { OnboardingButton } from "./shared";

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
import { buildWebAppUrl } from "~/shared/utils";

const GOOGLE_PROVIDER = PROVIDERS.find((provider) => provider.id === "google");
const OUTLOOK_PROVIDER = PROVIDERS.find(
  (provider) => provider.id === "outlook",
);

async function openOnboardingIntegrationUrl(
  nangoIntegrationId: string | undefined,
  connectionId: string | undefined,
  action: "connect" | "reconnect" | "disconnect",
) {
  if (!nangoIntegrationId) return;

  const params: Record<string, string> = {
    action,
    integration_id: nangoIntegrationId,
  };

  if (connectionId) {
    params.connection_id = connectionId;
  }

  const url = await buildWebAppUrl("/app/integration", params);
  await openerCommands.openUrl(url, null);
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
      className="border-border/45 bg-card/28 rounded-xl border p-4 shadow-[inset_0_1px_0_rgba(255,255,255,0.4),0_8px_24px_-20px_rgba(87,83,78,0.35)] backdrop-blur-md backdrop-saturate-150"
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
        <div className="order-1 w-full basis-full">
          <AppleCalendarList />
        </div>
      )}

      <div className="order-2 flex min-w-56 flex-1">
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
          className="border-border bg-card text-foreground hover:bg-accent flex h-full w-full items-center justify-center gap-3 border px-6 shadow-[0_2px_6px_rgba(87,83,78,0.08),0_10px_18px_-10px_rgba(87,83,78,0.22)] transition-all duration-150"
        >
          <img
            src="/assets/apple-calendar.png"
            alt=""
            aria-hidden="true"
            className="size-6 rounded-[4px] object-cover"
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
      }),
    [connectionSourceMap, groups, providerConnections],
  );

  useMountEffect(() => {
    scheduleSync();
  });

  return (
    <div className="flex flex-col gap-3">
      {reconnectRequiredConnections.length > 0 && (
        <div className="flex items-start gap-2 text-sm text-amber-700">
          <span className="pt-1">
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
        className="border-border/45 bg-card/28 rounded-xl border p-4 shadow-[inset_0_1px_0_rgba(255,255,255,0.4),0_8px_24px_-20px_rgba(87,83,78,0.35)] backdrop-blur-md backdrop-saturate-150"
      />
    </div>
  );
}

function addIntegrationMenus({
  groups,
  connections,
  connectionSourceMap,
  provider,
}: {
  groups: CalendarGroup[];
  connections: ConnectionItem[];
  connectionSourceMap: Map<string, string>;
  provider: (typeof PROVIDERS)[number];
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
      }),
    [connectionSourceMap, groups, providerConnections],
  );

  useMountEffect(() => {
    scheduleSync();
  });

  return (
    <div className="flex flex-col gap-3">
      {reconnectRequiredConnections.length > 0 && (
        <div className="flex items-start gap-2 text-sm text-amber-700">
          <span className="pt-1">
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
        className="border-border/45 bg-card/28 rounded-xl border p-4 shadow-[inset_0_1px_0_rgba(255,255,255,0.4),0_8px_24px_-20px_rgba(87,83,78,0.35)] backdrop-blur-md backdrop-saturate-150"
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
    <div className="order-2 flex min-w-56 flex-1">
      <OnboardingButton
        onClick={onConnect}
        onMouseEnter={() => onHoverChange(true)}
        onMouseLeave={() => onHoverChange(false)}
        onFocus={() => onHoverChange(true)}
        onBlur={() => onHoverChange(false)}
        disabled={isOpening || (isSignedIn && (isPending || !isReady))}
        className={
          isSignedIn
            ? "border-border bg-card text-foreground hover:bg-accent disabled:hover:bg-card flex h-full w-full items-center justify-center gap-3 border shadow-[0_2px_6px_rgba(87,83,78,0.08),0_10px_18px_-10px_rgba(87,83,78,0.22)] disabled:cursor-not-allowed disabled:opacity-60"
            : "border-border bg-muted text-foreground hover:border-primary hover:bg-primary hover:text-primary-foreground focus-visible:border-primary focus-visible:bg-primary focus-visible:text-primary-foreground h-full w-full border-1 shadow-[0_2px_6px_rgba(87,83,78,0.01),0_10px_18px_-10px_rgba(87,83,78,0.1)] transition-all duration-150"
        }
      >
        {!isSignedIn ? (
          <span className="grid items-center overflow-hidden">
            <span className="invisible col-start-1 row-start-1 flex items-center justify-center gap-3">
              <Trans>Sign in to connect</Trans>
            </span>

            <motion.span
              className="col-start-1 row-start-1 flex items-center justify-center gap-3"
              animate={{ y: isHovered ? "100%" : "0%" }}
              transition={{ type: "spring", bounce: 0.15, duration: 0.35 }}
            >
              {provider.icon}
              <span className="text-md text-foreground font-normal">
                {provider.displayName}
              </span>
            </motion.span>

            <motion.span
              className="col-start-1 row-start-1 flex items-center justify-center gap-3"
              animate={{ y: isHovered ? "0%" : "-140%" }}
              transition={{ type: "spring", bounce: 0.15, duration: 0.35 }}
            >
              <Trans>Sign in to connect</Trans>
            </motion.span>
          </span>
        ) : (
          <>
            {isOpening ? (
              <Loader2Icon
                className="size-4 shrink-0 animate-spin"
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
    ).finally(() => {
      openInFlightRef.current = false;
      setIsOpening(false);
    });
  }, [auth.session, isPro, onSignIn, upgradeToPro]);

  if (!OUTLOOK_PROVIDER) {
    return null;
  }

  if (isError) {
    return (
      <p className="order-2 min-w-56 flex-1 text-sm text-red-600">
        <Trans>Failed to load Outlook Calendar</Trans>
      </p>
    );
  }

  const isSignedIn = !!auth.session;
  const isConnected = providerConnections.length > 0;

  return (
    <>
      {isConnected && (
        <div className="order-1 w-full basis-full">
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
    ).finally(() => {
      openInFlightRef.current = false;
      setIsOpening(false);
    });
  }, [auth.session, isPro, onSignIn, upgradeToPro]);

  if (!GOOGLE_PROVIDER) {
    return null;
  }

  if (isError) {
    return (
      <p className="order-2 min-w-56 flex-1 text-sm text-red-600">
        <Trans>Failed to load Google Calendar</Trans>
      </p>
    );
  }

  const isSignedIn = !!auth.session;
  const isConnected = providerConnections.length > 0;

  return (
    <>
      {isConnected && (
        <div className="order-1 w-full basis-full">
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
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-stretch gap-3">
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
          className="text-muted-foreground text-sm"
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
