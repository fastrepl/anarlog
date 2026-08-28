import { t } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import { CircleNotch, Copy, Key, LockSimple } from "@phosphor-icons/react";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";
import { Switch } from "@anlg/ui/components/ui/switch";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { ApiKeyRow } from "./api-key-row";
import { copyText } from "./clipboard";

import { useBillingAccess } from "~/auth/billing-context";
import {
  backfillCloudApiSnapshots,
  createCloudApiKey,
  getCloudApiSettings,
  listCloudApiKeys,
  revokeCloudApiKey,
  scheduleCloudApiBackfillRetry,
  setCloudApiEnabled,
  type CloudApiKey,
} from "~/cloud-api/client";
import { env } from "~/env";

const CLOUD_API_BASE_URL = new URL("/v1", env.VITE_API_URL).toString();
const CLOUD_MCP_URL = new URL("/mcp", env.VITE_API_URL).toString();
const CLOUD_API_SETTINGS_QUERY_KEY = ["cloud-api", "settings"] as const;
const CLOUD_API_KEYS_QUERY_KEY = ["cloud-api", "keys"] as const;

export function CloudApiSection() {
  const billing = useBillingAccess();
  const queryClient = useQueryClient();
  const settingsQuery = useQuery({
    queryKey: CLOUD_API_SETTINGS_QUERY_KEY,
    queryFn: getCloudApiSettings,
    enabled: billing.isReady && billing.isPro,
    retry: false,
  });
  const toggleMutation = useMutation({
    mutationFn: async (enabled: boolean) => {
      const settings = await setCloudApiEnabled(enabled);
      try {
        const uploaded = enabled ? await backfillCloudApiSnapshots() : 0;
        return { settings, uploaded, backfillFailed: false };
      } catch {
        scheduleCloudApiBackfillRetry();
        return { settings, uploaded: 0, backfillFailed: enabled };
      }
    },
    onSuccess: ({ settings, uploaded, backfillFailed }) => {
      queryClient.setQueryData(CLOUD_API_SETTINGS_QUERY_KEY, settings);
      void queryClient.invalidateQueries({
        queryKey: CLOUD_API_KEYS_QUERY_KEY,
      });
      if (backfillFailed) {
        sonnerToast.error(
          t`Cloud API enabled, but existing meetings could not be uploaded. Anarlog will retry.`,
        );
      } else if (settings.enabled) {
        sonnerToast.success(
          uploaded === 1
            ? t`Cloud API enabled — 1 meeting uploaded`
            : t`Cloud API enabled — ${uploaded} meetings uploaded`,
        );
      } else {
        sonnerToast.success(t`Cloud API disabled and readable copies deleted`);
      }
    },
    onError: (error) => sonnerToast.error(error.message),
  });
  const enabled = settingsQuery.data?.enabled === true;

  if (!billing.isReady) {
    return (
      <section className="flex items-start justify-between gap-4">
        <CloudApiHeading />
        <CircleNotch
          aria-label={t`Loading Cloud API access`}
          className="text-muted-foreground mt-1 size-4 animate-spin"
        />
      </section>
    );
  }

  if (!billing.isPro) {
    return (
      <section className="flex items-start justify-between gap-6">
        <div className="flex gap-3">
          <LockSimple className="text-muted-foreground mt-1 size-4 shrink-0" />
          <div>
            <h2 className="font-sans text-lg font-semibold">
              <Trans>Cloud API & Connectors</Trans>
            </h2>
            <p className="text-muted-foreground mt-1 text-xs leading-5">
              <Trans>
                Access meetings remotely through the REST API and MCP connectors
                with Anarlog Pro.
              </Trans>
            </p>
          </div>
        </div>
        <Button
          type="button"
          size="sm"
          onClick={billing.upgradeToPro}
          disabled={billing.isUpgradingToPro}
        >
          {billing.isUpgradingToPro ? (
            <CircleNotch className="size-4 animate-spin" />
          ) : null}
          <Trans>Upgrade to Pro</Trans>
        </Button>
      </section>
    );
  }

  return (
    <section className="flex flex-col gap-4">
      <div className="flex items-start justify-between gap-4">
        <CloudApiHeading error={settingsQuery.error?.message} />
        <Switch
          checked={enabled}
          aria-label={t`Enable Cloud API & Connectors`}
          disabled={
            settingsQuery.isPending ||
            settingsQuery.isError ||
            toggleMutation.isPending
          }
          onCheckedChange={(checked) => toggleMutation.mutate(checked)}
        />
      </div>

      {enabled && (
        <>
          <div className="grid gap-3 sm:grid-cols-2">
            <CloudEndpoint
              label={t`REST API`}
              value={CLOUD_API_BASE_URL}
              copyMessage={t`Cloud API URL copied`}
            />
            <CloudEndpoint
              label={t`Remote MCP`}
              value={CLOUD_MCP_URL}
              copyMessage={t`Remote MCP URL copied`}
            />
          </div>
          <CloudApiKeys />
        </>
      )}
    </section>
  );
}

function CloudApiHeading({ error }: { error?: string }) {
  return (
    <div className="min-w-0">
      <h2 className="font-sans text-lg font-semibold">
        <Trans>Cloud API & Connectors</Trans>
      </h2>
      <p className="text-muted-foreground mt-1 text-xs">
        <Trans>
          Uploads meeting content for remote access while Anarlog is closed.
        </Trans>
      </p>
      {error ? <p className="text-destructive mt-2 text-xs">{error}</p> : null}
    </div>
  );
}

function CloudEndpoint({
  label,
  value,
  copyMessage,
}: {
  label: string;
  value: string;
  copyMessage: string;
}) {
  return (
    <div className="min-w-0">
      <p className="text-muted-foreground text-xs font-medium">{label}</p>
      <div className="mt-1 flex items-center gap-1">
        <code className="bg-muted scrollbar-hide min-w-0 overflow-x-auto rounded-md px-1.5 py-0.5 text-xs">
          {value}
        </code>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="size-7 shrink-0 p-0"
          aria-label={t`Copy ${label} URL`}
          onClick={() => void copyText(value, copyMessage)}
        >
          <Copy className="size-3.5" />
        </Button>
      </div>
    </div>
  );
}

function CloudApiKeys() {
  const queryClient = useQueryClient();
  const keysQuery = useQuery({
    queryKey: CLOUD_API_KEYS_QUERY_KEY,
    queryFn: listCloudApiKeys,
  });
  const createMutation = useMutation({
    mutationFn: createCloudApiKey,
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: CLOUD_API_KEYS_QUERY_KEY }),
    onError: (error) => sonnerToast.error(error.message),
  });
  const revokeMutation = useMutation({
    mutationFn: revokeCloudApiKey,
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: CLOUD_API_KEYS_QUERY_KEY }),
    onError: (error) => sonnerToast.error(error.message),
  });
  const form = useForm({
    defaultValues: { name: "" },
    onSubmit: ({ value }) => {
      createMutation.mutate(value.name.trim() || t`Connector`);
      form.setFieldValue("name", "");
    },
  });
  const createdKey = createMutation.data;
  const keys = keysQuery.data ?? [];

  return (
    <div>
      <div className="mb-3 flex items-center gap-2">
        <Key className="text-muted-foreground size-4" />
        <h4 className="text-sm font-medium">
          <Trans>Cloud API keys</Trans>
        </h4>
      </div>
      <form
        className="flex gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          event.stopPropagation();
          void form.handleSubmit();
        }}
      >
        <form.Field name="name">
          {(field) => (
            <Input
              className="h-8 max-w-64 text-sm"
              placeholder={t`Key name (e.g. Claude Code)`}
              value={field.state.value}
              onChange={(event) => field.handleChange(event.target.value)}
            />
          )}
        </form.Field>
        <Button
          type="submit"
          size="sm"
          variant="outline"
          disabled={createMutation.isPending}
        >
          <Trans>Create key</Trans>
        </Button>
      </form>

      {createdKey && (
        <div className="border-border bg-muted/30 mt-3 rounded-xl border p-3">
          <p className="text-muted-foreground text-xs">
            <Trans>Copy this key now — it is only shown once.</Trans>
          </p>
          <div className="mt-2 flex items-center gap-2">
            <code className="bg-muted scrollbar-hide overflow-x-auto rounded-md px-1.5 py-0.5 text-xs">
              {createdKey.key}
            </code>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-7 shrink-0"
              onClick={async () => {
                if (await copyText(createdKey.key, t`Cloud API key copied`)) {
                  createMutation.reset();
                }
              }}
            >
              <Copy className="size-3.5" />
              <Trans>Copy</Trans>
            </Button>
          </div>
        </div>
      )}

      {keys.length > 0 && (
        <ul className="mt-3 flex flex-col gap-1.5">
          {keys.map((key) => (
            <CloudApiKeyRow
              key={key.id}
              apiKey={key}
              onRevoke={() => revokeMutation.mutate(key.id)}
            />
          ))}
        </ul>
      )}
    </div>
  );
}

function CloudApiKeyRow({
  apiKey,
  onRevoke,
}: {
  apiKey: CloudApiKey;
  onRevoke: () => void;
}) {
  return <ApiKeyRow apiKey={apiKey} onRevoke={onRevoke} />;
}
