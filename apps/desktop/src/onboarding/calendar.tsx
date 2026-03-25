import { platform } from "@tauri-apps/plugin-os";
import { PlusIcon } from "lucide-react";
import { useCallback, useMemo } from "react";

import type { ConnectionItem } from "@hypr/api-client";
import { commands as openerCommands } from "@hypr/plugin-opener2";

import { OnboardingButton } from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { useConnections } from "~/auth/useConnections";
import { useAppleCalendarSelection } from "~/calendar/components/apple/calendar-selection";
import { TroubleShootingLink } from "~/calendar/components/apple/permission";
import { CalendarSelection } from "~/calendar/components/calendar-selection";
import { SyncProvider, useSync } from "~/calendar/components/context";
import { useOAuthCalendarSelection } from "~/calendar/components/oauth/calendar-selection";
import {
  type ConnectionAction,
  ConnectionTroubleShootingLink,
  ReconnectRequiredIndicator,
} from "~/calendar/components/oauth/status";
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

function AppleCalendarList() {
  const { scheduleSync } = useSync();
  const { groups, handleToggle, isLoading } = useAppleCalendarSelection();

  useMountEffect(() => {
    scheduleSync();
  });

  return (
    <CalendarSelection
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
  return (
    <div className="flex flex-col gap-3">
      {isAuthorized ? (
        <AppleCalendarList />
      ) : (
        <div className="flex items-center gap-3">
          <OnboardingButton
            onClick={onRequest}
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
          <TroubleShootingLink
            onRequest={onRequest}
            onReset={onReset}
            onOpen={onOpen}
            isPending={isPending}
            className="text-sm text-neutral-500"
          />
        </div>
      )}
    </div>
  );
}

function GoogleCalendarConnectedContent({
  connections,
}: {
  connections: ConnectionItem[];
}) {
  const { scheduleSync } = useSync();
  const { groups, connectionSourceMap, handleToggle, isLoading } =
    useOAuthCalendarSelection(GOOGLE_PROVIDER!);

  useMountEffect(() => {
    scheduleSync();
  });

  const connectionActions = useMemo(
    (): ConnectionAction[] =>
      connections.map((connection) => ({
        connectionId: connection.connection_id,
        label:
          connectionSourceMap.get(connection.connection_id) ??
          connection.connection_id,
        onReconnect: () =>
          void openOnboardingIntegrationUrl(
            GOOGLE_PROVIDER?.nangoIntegrationId,
            connection.connection_id,
            "reconnect",
          ),
        onDisconnect: () =>
          void openOnboardingIntegrationUrl(
            GOOGLE_PROVIDER?.nangoIntegrationId,
            connection.connection_id,
            "disconnect",
          ),
      })),
    [connectionSourceMap, connections],
  );

  return (
    <div className="flex flex-col gap-3">
      <ConnectionTroubleShootingLink connections={connectionActions} />

      <CalendarSelection
        groups={groups}
        onToggle={handleToggle}
        isLoading={isLoading}
        disableHoverTone
        className="rounded-xl border border-white/45 bg-white/28 shadow-[inset_0_1px_0_rgba(255,255,255,0.4),0_8px_24px_-20px_rgba(87,83,78,0.35)] backdrop-blur-md backdrop-saturate-150"
      />

      <button
        type="button"
        onClick={() =>
          void openOnboardingIntegrationUrl(
            GOOGLE_PROVIDER?.nangoIntegrationId,
            undefined,
            "connect",
          )
        }
        className="flex w-fit cursor-pointer items-center gap-1 text-xs text-neutral-600 underline transition-colors hover:text-neutral-900"
      >
        <PlusIcon className="size-3" />
        Add another account
      </button>
    </div>
  );
}

function GoogleCalendarProvider() {
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
  const reconnectRequired = providerConnections.filter(
    (connection) => connection.status === "reconnect_required",
  );

  const handleConnect = useCallback(() => {
    if (!auth.session) {
      void auth.signIn();
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
  }, [auth, isPro, upgradeToPro]);

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
      <div className="flex flex-col gap-3">
        {reconnectRequired.map((connection) => (
          <div
            key={connection.connection_id}
            className="flex flex-col gap-2 rounded-xl border border-amber-200/70 bg-amber-50/70 px-4 py-3"
          >
            <div className="flex items-center gap-2 text-sm text-amber-800">
              <ReconnectRequiredIndicator />
              <span>Reconnect required for Google Calendar</span>
            </div>

            {connection.last_error_description && (
              <p className="text-sm text-amber-900/80">
                {connection.last_error_description}
              </p>
            )}

            <div className="flex items-center gap-2 text-sm">
              <button
                type="button"
                onClick={() =>
                  void openOnboardingIntegrationUrl(
                    GOOGLE_PROVIDER.nangoIntegrationId,
                    connection.connection_id,
                    "reconnect",
                  )
                }
                className="underline transition-colors hover:text-neutral-900"
              >
                Reconnect
              </button>
              <span className="text-neutral-400">or</span>
              <button
                type="button"
                onClick={() =>
                  void openOnboardingIntegrationUrl(
                    GOOGLE_PROVIDER.nangoIntegrationId,
                    connection.connection_id,
                    "disconnect",
                  )
                }
                className="text-red-500 underline transition-colors hover:text-red-700"
              >
                Disconnect
              </button>
            </div>
          </div>
        ))}

        <GoogleCalendarConnectedContent connections={providerConnections} />
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      <OnboardingButton
        onClick={handleConnect}
        disabled={isPending || (auth.session !== null && !isReady)}
        className="flex items-center gap-3 border border-neutral-200 bg-white text-stone-800 shadow-[0_2px_6px_rgba(87,83,78,0.08),0_10px_18px_-10px_rgba(87,83,78,0.22)] hover:bg-stone-50"
      >
        {GOOGLE_PROVIDER.icon}
        Connect Google Calendar
      </OnboardingButton>

      {!auth.session && (
        <p className="text-sm text-neutral-500">
          Sign in to connect your Google account.
        </p>
      )}

      {auth.session && !isPro && isReady && (
        <p className="text-sm text-neutral-500">
          Google Calendar is available on Char Pro.
        </p>
      )}
    </div>
  );
}

function CalendarSectionContent({ onContinue }: { onContinue: () => void }) {
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

      <GoogleCalendarProvider />

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

export function CalendarSection({ onContinue }: { onContinue: () => void }) {
  return (
    <SyncProvider>
      <CalendarSectionContent onContinue={onContinue} />
    </SyncProvider>
  );
}
