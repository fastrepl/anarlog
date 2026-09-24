import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useRef, useState } from "react";

import { CircleNotch } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";

import {
  cancelCrmConnection,
  connectCrm,
  crmCredentialsQueryKey,
  crmCredentialsQueryOptions,
  type CrmProviderInfo,
  crmProvidersQueryOptions,
  disconnectCrm,
  verifyCrmConnection,
} from "~/crm/connection";
import { SettingsPageTitle } from "~/settings/page-title";

export function SettingsCrm() {
  const providers = useQuery(crmProvidersQueryOptions());

  return (
    <div className="flex flex-col gap-6">
      <SettingsPageTitle title={<Trans>CRM</Trans>} />
      <p className="text-muted-foreground text-sm">
        <Trans>
          Connect a CRM to look up your contacts from Anarlog. Access tokens
          stay on this device.
        </Trans>
      </p>
      {providers.data?.length === 0 ? (
        <p className="text-muted-foreground text-sm">
          <Trans>No CRM integrations are available yet.</Trans>
        </p>
      ) : (
        <div className="divide-border flex flex-col divide-y">
          {providers.data?.map((provider) => (
            <CrmProviderRow key={provider.id} provider={provider} />
          ))}
        </div>
      )}
    </div>
  );
}

function CrmProviderRow({ provider }: { provider: CrmProviderInfo }) {
  const { t } = useLingui();
  const queryClient = useQueryClient();
  const credentials = useQuery(crmCredentialsQueryOptions(provider.id));
  const abortController = useRef<AbortController | null>(null);
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");

  const connectMutation = useMutation({
    mutationKey: ["crm", provider.id, "connect"],
    mutationFn: async () => {
      const controller = new AbortController();
      abortController.current = controller;
      try {
        return await connectCrm(
          provider,
          provider.requiresClient
            ? { clientId, clientSecret: clientSecret || null }
            : null,
          controller.signal,
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
      setClientId("");
      setClientSecret("");
      queryClient.setQueryData(crmCredentialsQueryKey(provider.id), result);
    },
  });

  const cancelMutation = useMutation({
    mutationFn: () => {
      abortController.current?.abort();
      return cancelCrmConnection(provider.id);
    },
  });

  const verifyMutation = useMutation({
    mutationFn: () => verifyCrmConnection(provider),
    onSuccess: (result) => {
      queryClient.setQueryData(crmCredentialsQueryKey(provider.id), result);
    },
  });

  const disconnectMutation = useMutation({
    mutationFn: () => disconnectCrm(provider.id),
    onSuccess: () => {
      verifyMutation.reset();
      queryClient.setQueryData(crmCredentialsQueryKey(provider.id), null);
    },
  });

  const connected = Boolean(credentials.data);
  const error =
    connectMutation.error ?? verifyMutation.error ?? disconnectMutation.error;

  return (
    <div className="flex flex-col gap-3 py-4">
      <div className="flex items-center justify-between gap-4">
        <div className="flex flex-col gap-0.5">
          <span className="text-sm font-medium">{provider.name}</span>
          <span className="text-muted-foreground text-xs">
            {credentials.isPending ? (
              <Trans>Checking connection…</Trans>
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
              onClick={() => cancelMutation.mutate()}
            >
              <CircleNotch className="size-3 animate-spin" />
              <Trans>Cancel</Trans>
            </Button>
          ) : (
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={
                credentials.isPending ||
                (provider.requiresClient && clientId.trim().length === 0)
              }
              onClick={() => connectMutation.mutate()}
            >
              <Trans>Connect</Trans>
            </Button>
          )}
        </div>
      </div>
      {!connected && provider.requiresClient && provider.redirectUri ? (
        <div className="flex flex-col gap-2">
          <p className="text-muted-foreground text-xs">
            <Trans>
              Create an OAuth app in {provider.name} with redirect URL{" "}
              <code className="select-all">{provider.redirectUri}</code> and
              paste its credentials here.
            </Trans>
          </p>
          <div className="grid grid-cols-2 gap-2">
            <Input
              value={clientId}
              onChange={(event) => setClientId(event.target.value)}
              placeholder={t`Client ID`}
              aria-label={t`${provider.name} client ID`}
              autoComplete="off"
              disabled={connectMutation.isPending}
            />
            <Input
              type="password"
              value={clientSecret}
              onChange={(event) => setClientSecret(event.target.value)}
              placeholder={t`Client secret (optional)`}
              aria-label={t`${provider.name} client secret`}
              autoComplete="off"
              disabled={connectMutation.isPending}
            />
          </div>
        </div>
      ) : null}
      {error ? (
        <p className="text-xs text-red-600" role="alert">
          {error.message}
        </p>
      ) : null}
    </div>
  );
}
