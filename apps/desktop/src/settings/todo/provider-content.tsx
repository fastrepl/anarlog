import { Trans, useLingui } from "@lingui/react/macro";
import { useCallback, useMemo } from "react";

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";

import { TodoFilterField, TODO_FILTER_SETTING_KEYS } from "./filter-field";
import { GitHubTodoProviderContent } from "./github";
import type { TodoProvider } from "./shared";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { useConnections } from "~/auth/useConnections";
import {
  AccessPermissionRow,
  TroubleShootingLink,
} from "~/calendar/components/apple/permission";
import { usePermission } from "~/shared/hooks/usePermissions";
import { openIntegrationUrl } from "~/shared/integration";

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
  const { isPaid, upgradeToPro } = useBillingAccess();
  const { data: connections, isError } = useConnections(isPaid);

  const providerConnections = useMemo(
    () =>
      connections?.filter(
        (connection) => connection.integration_id === config.nangoIntegrationId,
      ) ?? [],
    [connections, config.nangoIntegrationId],
  );

  const handleConnect = useCallback(
    () =>
      openIntegrationUrl(
        config.nangoIntegrationId,
        undefined,
        "connect",
        "todo",
      ),
    [config.nangoIntegrationId],
  );

  if (!auth.session) {
    return (
      <div className="pt-1 pb-2">
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <span
              tabIndex={0}
              className="text-muted-foreground cursor-not-allowed text-xs opacity-50"
            >
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
      <div className="pt-1 pb-2">
        <button
          type="button"
          onClick={upgradeToPro}
          className="text-muted-foreground hover:text-foreground cursor-pointer text-xs underline transition-colors"
        >
          <Trans>Upgrade to connect</Trans>
        </button>
      </div>
    );
  }

  if (isError) {
    return (
      <div className="pt-1 pb-2">
        <span className="text-xs text-red-600">
          <Trans>Failed to load integration status</Trans>
        </span>
      </div>
    );
  }

  if (providerConnections.length === 0) {
    return (
      <div className="pt-1 pb-2">
        <button
          type="button"
          onClick={handleConnect}
          className="text-muted-foreground hover:text-foreground cursor-pointer text-xs underline transition-colors"
        >
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
    <div className="flex flex-col gap-3">
      <ConnectionActions
        config={config}
        providerConnections={providerConnections}
      />
      {filterSettingKey ? (
        <TodoFilterField
          settingKey={filterSettingKey}
          label={config.filterLabel ?? "Repository"}
          description={t`Filter synced items by ${(config.filterLabel ?? "repository").toLowerCase()}.`}
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
      <div className="flex items-center gap-2 pb-1">
        <button
          type="button"
          onClick={() =>
            openIntegrationUrl(
              config.nangoIntegrationId,
              activeConnection.connection_id,
              "reconnect",
              "todo",
            )
          }
          className="cursor-pointer text-xs text-amber-700 underline transition-colors hover:text-amber-900"
        >
          <Trans>Reconnect required</Trans>
        </button>
        <span className="text-muted-foreground text-xs">
          <Trans>or</Trans>
        </span>
        <button
          type="button"
          onClick={() =>
            openIntegrationUrl(
              config.nangoIntegrationId,
              activeConnection.connection_id,
              "disconnect",
              "todo",
            )
          }
          className="cursor-pointer text-xs text-red-500 underline transition-colors hover:text-red-700"
        >
          <Trans>Disconnect</Trans>
        </button>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2 pb-1">
      <button
        type="button"
        onClick={() =>
          openIntegrationUrl(
            config.nangoIntegrationId,
            activeConnection.connection_id,
            "disconnect",
            "todo",
          )
        }
        className="text-muted-foreground hover:text-muted-foreground cursor-pointer text-xs underline transition-colors"
      >
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
