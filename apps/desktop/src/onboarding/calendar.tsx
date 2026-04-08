import { platform } from "@tauri-apps/plugin-os";
import { motion } from "motion/react";
import { useCallback, useMemo, useState } from "react";

import type { ConnectionItem } from "@hypr/api-client";
import { commands as openerCommands } from "@hypr/plugin-opener2";

import { OnboardingButton, OnboardingCharIcon } from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
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
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { usePermission } from "~/shared/hooks/usePermissions";
import { buildWebAppUrl } from "~/shared/utils";
import * as main from "~/store/tinybase/store/main";

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
  const { groups, handleToggle, isLoading } = useAppleCalendarSelection();

  useMountEffect(() => {
    scheduleSync();
  });

  return (
    <CalendarSelection
      key={getCalendarSelectionKey(groups)}
      groups={groups}
      onToggle={handleToggle}
      isLoading={isLoading}
      disableHoverTone
      className="rounded-xl border border-white/45 bg-white/28 shadow-[inset_0_1px_0_rgba(255,255,255,0.4),0_8px_24px_-20px_rgba(87,83,78,0.35)] backdrop-blur-md backdrop-saturate-150"
    />
  );
}

function AppleCalendarProvider({
  isAuthorized,
  isPending,
  onRequest,
  onTroubleshoot,
}: {
  isAuthorized: boolean;
  isPending: boolean;
  onRequest: () => void;
  onTroubleshoot: () => void;
}) {
  return (
    <div className="flex flex-col gap-3">
      {isAuthorized ? (
        <AppleCalendarList />
      ) : (
        <OnboardingButton
          onClick={() => {
            onTroubleshoot();
            onRequest();
          }}
          disabled={isPending}
          className="flex h-full w-full items-center justify-center gap-3 border border-neutral-200 bg-white px-12 text-stone-800 shadow-[0_2px_6px_rgba(87,83,78,0.08),0_10px_18px_-10px_rgba(87,83,78,0.22)] transition-all duration-150 hover:bg-stone-100"
        >
          <img
            src="/assets/apple-calendar.png"
            alt=""
            aria-hidden="true"
            className="size-6 rounded-[4px] object-cover"
          />
          Apple
        </OnboardingButton>
      )}
    </div>
  );
}

function GoogleCalendarConnectedContent({
  providerConnections,
}: {
  providerConnections: ConnectionItem[];
}) {
  const { scheduleSync } = useSync();
  const { groups, connectionSourceMap, handleToggle, isLoading } =
    useOAuthCalendarSelection(GOOGLE_PROVIDER!);
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
        isLoading={isLoading}
        disableHoverTone
        className="rounded-xl border border-white/45 bg-white/28 shadow-[inset_0_1px_0_rgba(255,255,255,0.4),0_8px_24px_-20px_rgba(87,83,78,0.35)] backdrop-blur-md backdrop-saturate-150"
      />

      <OnboardingButton
        type="button"
        onClick={() =>
          void openOnboardingIntegrationUrl(
            GOOGLE_PROVIDER?.nangoIntegrationId,
            undefined,
            "connect",
          )
        }
        className="flex items-center gap-3 border border-neutral-200 bg-white text-stone-800 shadow-[0_2px_6px_rgba(87,83,78,0.08),0_10px_18px_-10px_rgba(87,83,78,0.22)] hover:bg-stone-50"
      >
        {GOOGLE_PROVIDER?.icon}
        Add another account
      </OnboardingButton>
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
  const { groups, connectionSourceMap, handleToggle, isLoading } =
    useOAuthCalendarSelection(OUTLOOK_PROVIDER!);
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
        isLoading={isLoading}
        disableHoverTone
        className="rounded-xl border border-white/45 bg-white/28 shadow-[inset_0_1px_0_rgba(255,255,255,0.4),0_8px_24px_-20px_rgba(87,83,78,0.35)] backdrop-blur-md backdrop-saturate-150"
      />

      <OnboardingButton
        type="button"
        onClick={() =>
          void openOnboardingIntegrationUrl(
            OUTLOOK_PROVIDER?.nangoIntegrationId,
            undefined,
            "connect",
          )
        }
        className="flex items-center gap-3 border border-neutral-200 bg-white text-stone-800 shadow-[0_2px_6px_rgba(87,83,78,0.08),0_10px_18px_-10px_rgba(87,83,78,0.22)] hover:bg-stone-50"
      >
        {OUTLOOK_PROVIDER?.icon}
        Add another account
      </OnboardingButton>
    </div>
  );
}

function OutlookCalendarProvider({ onSignIn }: { onSignIn: () => void }) {
  const auth = useAuth();
  const { isPro, isReady, upgradeToPro } = useBillingAccess();
  const { data: connections, isPending, isError } = useConnections(isPro);
  const [isHovered, setHovered] = useState(false);
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

    void openOnboardingIntegrationUrl(
      OUTLOOK_PROVIDER?.nangoIntegrationId,
      undefined,
      "connect",
    );
  }, [auth.session, isPro, onSignIn, upgradeToPro]);

  if (!OUTLOOK_PROVIDER) {
    return null;
  }

  if (isError) {
    return (
      <p className="text-sm text-red-600">Failed to load Outlook Calendar</p>
    );
  }

  if (providerConnections.length > 0) {
    return (
      <OutlookCalendarConnectedContent
        providerConnections={providerConnections}
      />
    );
  }

  const isSignedIn = !!auth.session;

  return (
    <OnboardingButton
      onClick={handleConnect}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onFocus={() => setHovered(true)}
      onBlur={() => setHovered(false)}
      disabled={
        isSignedIn && (isPending || (auth.session !== null && !isReady))
      }
      className={
        isSignedIn
          ? "gho flex items-center gap-3 border border-neutral-200 bg-white text-stone-800 shadow-[0_2px_6px_rgba(87,83,78,0.08),0_10px_18px_-10px_rgba(87,83,78,0.22)] hover:bg-stone-100 disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:bg-white"
          : "border-1 border-neutral-200 bg-gray-100 text-stone-800 shadow-[0_2px_6px_rgba(87,83,78,0.01),0_10px_18px_-10px_rgba(87,83,78,0.1)] transition-all duration-150 hover:border-stone-600 hover:bg-stone-800 hover:text-white focus-visible:border-stone-600 focus-visible:bg-stone-800 focus-visible:text-white"
      }
    >
      {!isSignedIn ? (
        <span className="grid items-center overflow-hidden">
          <span className="invisible col-start-1 row-start-1 flex items-center justify-center gap-3">
            <OnboardingCharIcon inverted />
            Sign in to Char
          </span>

          <motion.span
            className="col-start-1 row-start-1 flex items-center justify-center gap-3"
            animate={{ y: isHovered ? "100%" : "0%" }}
            transition={{ type: "spring", bounce: 0.15, duration: 0.35 }}
          >
            {OUTLOOK_PROVIDER.icon}
            <div className="flex flex-col items-start justify-center">
              <p className="text-md font-normal text-neutral-900">Outlook</p>
              <span className="text-xs font-normal text-neutral-500">
                Only in Pro
              </span>
            </div>
          </motion.span>

          <motion.span
            className="col-start-1 row-start-1 flex items-center justify-center gap-3"
            animate={{ y: isHovered ? "0%" : "-150%" }}
            transition={{ type: "spring", bounce: 0.15, duration: 0.35 }}
          >
            <OnboardingCharIcon inverted />
            Sign in to Char
          </motion.span>
        </span>
      ) : (
        <>
          {OUTLOOK_PROVIDER.icon}
          Connect Outlook
        </>
      )}
    </OnboardingButton>
  );
}

function GoogleCalendarProvider({ onSignIn }: { onSignIn: () => void }) {
  const auth = useAuth();
  const { isPro, isReady, upgradeToPro } = useBillingAccess();
  const { data: connections, isPending, isError } = useConnections(isPro);
  const [isHovered, setHovered] = useState(false);
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

    void openOnboardingIntegrationUrl(
      GOOGLE_PROVIDER?.nangoIntegrationId,
      undefined,
      "connect",
    );
  }, [auth.session, isPro, onSignIn, upgradeToPro]);

  if (!GOOGLE_PROVIDER) {
    return null;
  }

  if (isError) {
    return (
      <p className="text-sm text-red-600">Failed to load Google Calendar</p>
    );
  }

  if (providerConnections.length > 0) {
    return (
      <GoogleCalendarConnectedContent
        providerConnections={providerConnections}
      />
    );
  }

  const isSignedIn = !!auth.session;

  return (
    <div className="flex h-full items-center gap-3">
      <OnboardingButton
        onClick={handleConnect}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
        onFocus={() => setHovered(true)}
        onBlur={() => setHovered(false)}
        disabled={
          isSignedIn && (isPending || (auth.session !== null && !isReady))
        }
        className={
          isSignedIn
            ? "flex items-center gap-3 border border-neutral-200 bg-white text-stone-800 shadow-[0_2px_6px_rgba(87,83,78,0.08),0_10px_18px_-10px_rgba(87,83,78,0.22)] hover:bg-stone-50 disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:bg-white"
            : "border-1 border-neutral-200 bg-gray-100 text-stone-800 shadow-[0_2px_6px_rgba(87,83,78,0.01),0_10px_18px_-10px_rgba(87,83,78,0.1)] transition-all duration-150 hover:border-stone-600 hover:bg-stone-800 hover:text-white focus-visible:border-stone-600 focus-visible:bg-stone-800 focus-visible:text-white"
        }
      >
        {!isSignedIn ? (
          <span className="grid items-center overflow-hidden">
            <span className="invisible col-start-1 row-start-1 flex items-center justify-center gap-3">
              <OnboardingCharIcon inverted />
              Sign in to Char
            </span>

            <motion.span
              className="col-start-1 row-start-1 flex items-center justify-center gap-3"
              animate={{ y: isHovered ? "100%" : "0%" }}
              transition={{ type: "spring", bounce: 0.15, duration: 0.35 }}
            >
              {GOOGLE_PROVIDER.icon}
              <div className="flex flex-col items-start justify-center">
                <p className="text-md font-normal text-neutral-900">Google</p>
                <span className="text-xs font-normal text-neutral-500">
                  Only in Pro
                </span>
              </div>
            </motion.span>

            <motion.span
              className="col-start-1 row-start-1 flex items-center justify-center gap-3"
              animate={{ y: isHovered ? "0%" : "-140%" }}
              transition={{ type: "spring", bounce: 0.15, duration: 0.35 }}
            >
              <OnboardingCharIcon inverted />
              Sign in to Char
            </motion.span>
          </span>
        ) : (
          <>
            {GOOGLE_PROVIDER.icon}
            Connect Google Calendar
          </>
        )}
      </OnboardingButton>
    </div>
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
  const enabledCalendars = main.UI.useResultTable(
    main.QUERIES.enabledCalendars,
    main.STORE_ID,
  );
  const hasConnectedCalendar = Object.keys(enabledCalendars ?? {}).length > 0;

  const hasAnyConnected = hasConnectedCalendar || isAuthorized;

  return (
    <div className="flex flex-col gap-4">
      {hasAnyConnected ? (
        <div className="flex flex-col gap-4">
          {isMacos && (
            <AppleCalendarProvider
              isAuthorized={isAuthorized}
              isPending={calendar.isPending}
              onRequest={calendar.request}
              onTroubleshoot={() => setShowTroubleshooting(true)}
            />
          )}
          <GoogleCalendarProvider onSignIn={onSignIn} />
          <OutlookCalendarProvider onSignIn={onSignIn} />
        </div>
      ) : (
        // for the case when the user has no connected calendars yet we show the calendars in a row
        <div className="flex flex-row items-stretch gap-4">
          {isMacos && (
            <AppleCalendarProvider
              isAuthorized={isAuthorized}
              isPending={calendar.isPending}
              onRequest={calendar.request}
              onTroubleshoot={() => setShowTroubleshooting(true)}
            />
          )}

          <GoogleCalendarProvider onSignIn={onSignIn} />
          <OutlookCalendarProvider onSignIn={onSignIn} />
        </div>
      )}

      {showTroubleshooting && !isAuthorized && (
        <TroubleShootingLink
          onRequest={calendar.request}
          onReset={calendar.reset}
          onOpen={calendar.open}
          isPending={calendar.isPending}
          className="text-sm text-neutral-500"
        />
      )}

      {hasConnectedCalendar && (
        <OnboardingButton onClick={onContinue}>Continue</OnboardingButton>
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
