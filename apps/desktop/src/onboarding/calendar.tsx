import { platform } from "@tauri-apps/plugin-os";
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
  onOpen,
  onReset,
}: {
  isAuthorized: boolean;
  isPending: boolean;
  onRequest: () => void;
  onOpen: () => void;
  onReset: () => void;
}) {
  const [showTroubleshooting, setShowTroubleshooting] = useState(false);

  return (
    <div className="flex flex-col gap-3">
      {isAuthorized ? (
        <AppleCalendarList />
      ) : (
        <div className="flex items-center gap-3">
          <OnboardingButton
            onClick={() => {
              setShowTroubleshooting(true);
              onRequest();
            }}
            disabled={isPending}
            className="flex items-center gap-3 border border-neutral-200 bg-white text-stone-800 shadow-[0_2px_6px_rgba(87,83,78,0.08),0_10px_18px_-10px_rgba(87,83,78,0.22)] hover:bg-stone-50"
          >
            <img
              src="/assets/apple-calendar.png"
              alt=""
              aria-hidden="true"
              className="size-5 rounded-[4px] object-cover"
            />
            Connect Apple Calendar
          </OnboardingButton>
          {showTroubleshooting && (
            <TroubleShootingLink
              onRequest={onRequest}
              onReset={onReset}
              onOpen={onOpen}
              isPending={isPending}
              className="text-sm text-neutral-500"
            />
          )}
        </div>
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
}: {
  groups: CalendarGroup[];
  connections: ConnectionItem[];
  connectionSourceMap: Map<string, string>;
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
              GOOGLE_PROVIDER?.nangoIntegrationId,
              connection.connection_id,
              "reconnect",
            ),
        },
        {
          id: `disconnect-${connection.connection_id}`,
          text: "Disconnect",
          action: () =>
            void openOnboardingIntegrationUrl(
              GOOGLE_PROVIDER?.nangoIntegrationId,
              connection.connection_id,
              "disconnect",
            ),
        },
      ],
    };
  });
}

function GoogleCalendarProvider({ onSignIn }: { onSignIn: () => void }) {
  const auth = useAuth();
  const { isPro, isReady, upgradeToPro } = useBillingAccess();
  const { data: connections, isPending, isError } = useConnections(isPro);
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
    <div className="flex items-center gap-3">
      <OnboardingButton
        onClick={handleConnect}
        disabled={
          isSignedIn && (isPending || (auth.session !== null && !isReady))
        }
        className={
          isSignedIn
            ? "flex items-center gap-3 border border-neutral-200 bg-white text-stone-800 shadow-[0_2px_6px_rgba(87,83,78,0.08),0_10px_18px_-10px_rgba(87,83,78,0.22)] hover:bg-stone-50 disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:bg-white"
            : "group border-2 border-neutral-200 bg-white text-stone-800 shadow-[0_2px_6px_rgba(87,83,78,0.08),0_10px_18px_-10px_rgba(87,83,78,0.22)] hover:border-stone-600 hover:bg-stone-800 hover:text-white focus-visible:border-stone-600 focus-visible:bg-stone-800 focus-visible:text-white"
        }
      >
        {!isSignedIn ? (
          <span className="grid items-center">
            <span className="invisible col-start-1 row-start-1 flex items-center justify-center gap-3">
              {GOOGLE_PROVIDER.icon}
              Sign up to use Google
            </span>

            <span className="col-start-1 row-start-1 flex items-center justify-center gap-3 transition-opacity duration-150 group-hover:opacity-0 group-focus-visible:opacity-0">
              {GOOGLE_PROVIDER.icon}
              Sign up to use Google
            </span>

            <span className="col-start-1 row-start-1 flex items-center justify-center gap-3 opacity-0 transition-opacity duration-150 group-hover:opacity-100 group-focus-visible:opacity-100">
              <OnboardingCharIcon inverted />
              Sign in to Char
            </span>
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
  const enabledCalendars = main.UI.useResultTable(
    main.QUERIES.enabledCalendars,
    main.STORE_ID,
  );
  const hasConnectedCalendar = Object.keys(enabledCalendars ?? {}).length > 0;

  return (
    <div className="flex flex-col gap-4">
      {isMacos && (
        <AppleCalendarProvider
          isAuthorized={isAuthorized}
          isPending={calendar.isPending}
          onRequest={calendar.request}
          onOpen={calendar.open}
          onReset={calendar.reset}
        />
      )}

      <GoogleCalendarProvider onSignIn={onSignIn} />

      {hasConnectedCalendar ? (
        <OnboardingButton onClick={onContinue}>Continue</OnboardingButton>
      ) : (
        <button
          type="button"
          onClick={onContinue}
          className="w-fit text-sm text-neutral-500/70 transition-colors hover:text-neutral-700"
        >
          Skip
        </button>
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
