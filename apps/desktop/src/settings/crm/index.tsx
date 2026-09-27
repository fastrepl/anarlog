import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useRef } from "react";

import type { ConnectionItem } from "@anlg/api-client";
import {
  ArrowsClockwise,
  CaretDown,
  CircleNotch,
  PlugsConnected,
} from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  appFloatingMenuPanelClassName,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";

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
import { ConnectButtonGroup } from "~/shared/connect-button-group";

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
        <div className="border-border bg-card divide-border divide-y overflow-hidden rounded-2xl border">
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

function CrmProviderIcon({ provider }: { provider: CrmProviderInfo }) {
  if (provider.icon) {
    return (
      <img
        src={provider.icon}
        alt=""
        className="size-8 object-contain object-center"
      />
    );
  }

  return (
    <span
      className="bg-muted text-muted-foreground flex size-8 items-center justify-center rounded-lg text-xs font-semibold"
      aria-hidden="true"
    >
      {provider.name.charAt(0)}
    </span>
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
  const { t } = useLingui();
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
    <div>
      <div className="flex min-h-16 items-center gap-3 px-4 py-3">
        <span className="flex size-8 shrink-0 items-center justify-center">
          <CrmProviderIcon provider={provider} />
        </span>
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="truncate text-sm font-medium">{provider.name}</span>
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
        <div className="flex shrink-0 items-center gap-1">
          {connected ? (
            <ConnectButtonGroup
              primary
              busy={verifyMutation.isPending}
              busyLabel={t`Test`}
            >
              <Button
                type="button"
                size="sm"
                smoothCorners={false}
                aria-label={t`Test`}
                className="group/test hover:bg-primary-foreground/10 rounded-none border-0 bg-transparent shadow-none"
                disabled={
                  verifyMutation.isPending || disconnectMutation.isPending
                }
                aria-busy={verifyMutation.isPending}
                onClick={() => verifyMutation.mutate()}
              >
                {verifyMutation.isPending ? (
                  <CircleNotch className="size-3.5 animate-spin" />
                ) : (
                  <ArrowsClockwise className="size-3.5" />
                )}
                <span aria-hidden="true" className="grid">
                  <span className="col-start-1 row-start-1 group-hover/test:invisible group-focus-visible/test:invisible">
                    <Trans>Connected</Trans>
                  </span>
                  <span className="invisible col-start-1 row-start-1 group-hover/test:visible group-focus-visible/test:visible">
                    <Trans>Test</Trans>
                  </span>
                </span>
              </Button>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    type="button"
                    size="sm"
                    smoothCorners={false}
                    aria-label={t`More options`}
                    disabled={
                      verifyMutation.isPending || disconnectMutation.isPending
                    }
                    className="hover:bg-primary-foreground/10 before:bg-primary-foreground/20 relative w-6 rounded-none border-0 bg-transparent px-0 shadow-none before:absolute before:inset-y-1.5 before:left-0 before:w-px"
                  >
                    <CaretDown className="size-3.5" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent variant="app" align="end" className="w-40">
                  <AppFloatingPanel className={appFloatingMenuPanelClassName}>
                    <DropdownMenuItem
                      disabled={
                        verifyMutation.isPending || disconnectMutation.isPending
                      }
                      onClick={() => verifyMutation.mutate()}
                    >
                      <ArrowsClockwise />
                      <Trans>Test</Trans>
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      disabled={
                        verifyMutation.isPending || disconnectMutation.isPending
                      }
                      onClick={() => disconnectMutation.mutate()}
                    >
                      <Trans>Disconnect</Trans>
                    </DropdownMenuItem>
                  </AppFloatingPanel>
                </DropdownMenuContent>
              </DropdownMenu>
            </ConnectButtonGroup>
          ) : (
            <ConnectButtonGroup primary>
              <Button
                type="button"
                size="sm"
                variant="default"
                smoothCorners={false}
                disabled={!connectMutation.isPending && connectionsPending}
                className="hover:bg-primary-foreground/10 rounded-none border-0 bg-transparent shadow-none"
                onClick={() => {
                  if (connectMutation.isPending) {
                    abortController.current?.abort();
                    return;
                  }
                  connectMutation.mutate();
                }}
              >
                {connectMutation.isPending ? (
                  <CircleNotch className="size-3.5 animate-spin" />
                ) : (
                  <PlugsConnected className="size-3.5" />
                )}
                {connectMutation.isPending ? (
                  <Trans>Cancel</Trans>
                ) : reconnectRequired ? (
                  <Trans>Reconnect</Trans>
                ) : (
                  <Trans>Connect</Trans>
                )}
              </Button>
            </ConnectButtonGroup>
          )}
        </div>
      </div>
      {error ? (
        <p className="text-destructive px-4 pb-3 text-xs" role="alert">
          {error.message}
        </p>
      ) : null}
    </div>
  );
}
