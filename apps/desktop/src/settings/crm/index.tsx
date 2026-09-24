import { Trans } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useRef } from "react";

import type { ConnectionItem } from "@anlg/api-client";
import { CircleNotch } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";

import { useAuth } from "~/auth";
import { useConnections } from "~/auth/useConnections";
import {
  connectCrm,
  type CrmProviderInfo,
  crmProvidersQueryOptions,
  disconnectCrm,
  findCrmConnection,
  nangoConnectionIsReady,
  verifyCrmConnection,
} from "~/crm/connection";
import { SettingsPageTitle } from "~/settings/page-title";

export function SettingsCrm() {
  const auth = useAuth();
  const signedIn = Boolean(auth.session);
  const providers = useQuery(crmProvidersQueryOptions());
  const connections = useConnections(signedIn);

  return (
    <div className="flex flex-col gap-6">
      <SettingsPageTitle title={<Trans>CRM</Trans>} />
      <p className="text-muted-foreground text-sm">
        <Trans>
          Connect a CRM to look up your contacts from Anarlog. Connections are
          managed through your Anarlog account.
        </Trans>
      </p>
      {!signedIn ? (
        <p className="text-muted-foreground text-sm">
          <Trans>Sign in to connect a CRM.</Trans>
        </p>
      ) : providers.data?.length === 0 ? (
        <p className="text-muted-foreground text-sm">
          <Trans>No CRM integrations are available yet.</Trans>
        </p>
      ) : (
        <div className="divide-border flex flex-col divide-y">
          {providers.data?.map((provider) => (
            <CrmProviderRow
              key={provider.id}
              provider={provider}
              connection={findCrmConnection(provider, connections.data)}
              connectionsPending={connections.isPending}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function CrmProviderRow({
  provider,
  connection,
  connectionsPending,
}: {
  provider: CrmProviderInfo;
  connection: ConnectionItem | undefined;
  connectionsPending: boolean;
}) {
  const auth = useAuth();
  const queryClient = useQueryClient();
  const abortController = useRef<AbortController | null>(null);

  const invalidateConnections = () =>
    queryClient.invalidateQueries({
      queryKey: ["integration-status", auth.session?.user.id],
    });

  const connectMutation = useMutation({
    mutationKey: ["crm", provider.id, "connect"],
    mutationFn: async () => {
      const controller = new AbortController();
      abortController.current = controller;
      try {
        return await connectCrm(
          provider,
          auth.getHeaders()!,
          controller.signal,
          reconnectRequired ? connection?.connection_id : undefined,
        );
      } catch (error) {
        if (controller.signal.aborted) return null;
        throw error;
      } finally {
        if (abortController.current === controller) {
          abortController.current = null;
        }
      }
    },
    onSuccess: (result) => {
      if (!result) return;
      void invalidateConnections();
    },
  });

  const verifyMutation = useMutation({
    mutationFn: () =>
      verifyCrmConnection(
        provider,
        connection!.connection_id,
        auth.getHeaders()!,
      ),
  });

  const disconnectMutation = useMutation({
    mutationFn: () => disconnectCrm(provider, connection!.connection_id),
    onSuccess: () => {
      verifyMutation.reset();
      void invalidateConnections();
    },
  });

  const connected = Boolean(connection) && nangoConnectionIsReady(connection);
  const reconnectRequired =
    Boolean(connection) && !nangoConnectionIsReady(connection);
  const error =
    connectMutation.error ?? verifyMutation.error ?? disconnectMutation.error;

  return (
    <div className="flex flex-col gap-3 py-4">
      <div className="flex items-center justify-between gap-4">
        <div className="flex flex-col gap-0.5">
          <span className="text-sm font-medium">{provider.name}</span>
          <span className="text-muted-foreground text-xs">
            {connectionsPending ? (
              <Trans>Checking connection…</Trans>
            ) : reconnectRequired ? (
              <Trans>Reconnect required</Trans>
            ) : connected ? (
              verifyMutation.isSuccess ? (
                <Trans>Connected and working</Trans>
              ) : (
                <Trans>Connected</Trans>
              )
            ) : (
              <Trans>Not connected</Trans>
            )}
          </span>
        </div>
        <div className="flex items-center gap-2">
          {connected ? (
            <>
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={verifyMutation.isPending}
                onClick={() => verifyMutation.mutate()}
              >
                {verifyMutation.isPending && (
                  <CircleNotch className="size-3 animate-spin" />
                )}
                <Trans>Test</Trans>
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                disabled={disconnectMutation.isPending}
                onClick={() => disconnectMutation.mutate()}
              >
                <Trans>Disconnect</Trans>
              </Button>
            </>
          ) : connectMutation.isPending ? (
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => abortController.current?.abort()}
            >
              <CircleNotch className="size-3 animate-spin" />
              <Trans>Cancel</Trans>
            </Button>
          ) : (
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={connectionsPending}
              onClick={() => connectMutation.mutate()}
            >
              <Trans>{reconnectRequired ? "Reconnect" : "Connect"}</Trans>
            </Button>
          )}
        </div>
      </div>
      {error ? (
        <p className="text-xs text-red-600" role="alert">
          {error.message}
        </p>
      ) : null}
    </div>
  );
}
