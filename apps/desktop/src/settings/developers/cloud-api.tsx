import { ArrowSquareOut, Copy, Globe, Key } from "@phosphor-icons/react";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";
import { Switch } from "@anlg/ui/components/ui/switch";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { ApiKeyRow } from "./api-key-row";
import { copyText } from "./clipboard";

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

const CLOUD_API_GUIDE_URL = "https://docs.anarlog.so/reference/api-cloud";
const CLOUD_API_BASE_URL = new URL("/v1", env.VITE_API_URL).toString();
const CLOUD_MCP_URL = new URL("/mcp", env.VITE_API_URL).toString();
const CLOUD_API_SETTINGS_QUERY_KEY = ["cloud-api", "settings"] as const;
const CLOUD_API_KEYS_QUERY_KEY = ["cloud-api", "keys"] as const;

export function CloudApiSection() {
  const queryClient = useQueryClient();
  const settingsQuery = useQuery({
    queryKey: CLOUD_API_SETTINGS_QUERY_KEY,
    queryFn: getCloudApiSettings,
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
          "Cloud API enabled, but existing meetings could not be uploaded. Anarlog will retry.",
        );
      } else if (settings.enabled) {
        sonnerToast.success(
          uploaded === 1
            ? "Cloud API enabled — 1 meeting uploaded"
            : `Cloud API enabled — ${uploaded} meetings uploaded`,
        );
      } else {
        sonnerToast.success("Cloud API disabled and readable copies deleted");
      }
    },
    onError: (error) => sonnerToast.error(error.message),
  });
  const enabled = settingsQuery.data?.enabled === true;

  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-muted-foreground text-sm font-medium">
        Cloud API & Connectors
      </h2>
      <div className="border-border bg-card overflow-hidden rounded-2xl border">
        <div className="flex flex-col gap-4 p-4 sm:flex-row sm:items-start sm:justify-between">
          <div className="flex min-w-0 gap-3">
            <div className="bg-muted flex size-10 shrink-0 items-center justify-center rounded-xl">
              <Globe className="size-5" />
            </div>
            <div className="min-w-0">
              <h3 className="font-medium">Hosted access for agents</h3>
              <p className="text-muted-foreground mt-1 text-sm leading-5">
                Give remote agents meeting context while Anarlog is closed.
              </p>
              <p className="text-muted-foreground mt-2 text-xs leading-5">
                Enabling this uploads a separate server-readable copy of meeting
                titles, notes, summaries, participants, action items, and
                transcripts. Encrypted sync stays end-to-end encrypted. Turning
                this off deletes every readable copy from the server.
              </p>
              {settingsQuery.error && (
                <p className="text-destructive mt-2 text-xs">
                  {settingsQuery.error.message}
                </p>
              )}
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() =>
                void openerCommands.openUrl(CLOUD_API_GUIDE_URL, null)
              }
            >
              Guide
              <ArrowSquareOut className="size-3.5" />
            </Button>
            <Switch
              checked={enabled}
              aria-label="Enable Cloud API & Connectors"
              disabled={
                settingsQuery.isPending ||
                settingsQuery.isError ||
                toggleMutation.isPending
              }
              onCheckedChange={(checked) => toggleMutation.mutate(checked)}
            />
          </div>
        </div>

        {enabled && (
          <>
            <div className="border-border grid gap-3 border-t p-4 sm:grid-cols-2">
              <CloudEndpoint
                label="REST API"
                value={CLOUD_API_BASE_URL}
                copyMessage="Cloud API URL copied"
              />
              <CloudEndpoint
                label="Remote MCP"
                value={CLOUD_MCP_URL}
                copyMessage="Remote MCP URL copied"
              />
            </div>
            <CloudApiKeysCard />
          </>
        )}
      </div>
    </section>
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
          aria-label={`Copy ${label} URL`}
          onClick={() => void copyText(value, copyMessage)}
        >
          <Copy className="size-3.5" />
        </Button>
      </div>
    </div>
  );
}

function CloudApiKeysCard() {
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
      createMutation.mutate(value.name.trim() || "Connector");
      form.setFieldValue("name", "");
    },
  });
  const createdKey = createMutation.data;

  return (
    <div className="border-border border-t p-4">
      <div className="mb-3 flex items-center gap-2">
        <Key className="text-muted-foreground size-4" />
        <h4 className="text-sm font-medium">Cloud API keys</h4>
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
              placeholder="Key name (e.g. Claude Code)"
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
          Create key
        </Button>
      </form>

      {createdKey && (
        <div className="border-border bg-muted/30 mt-3 rounded-xl border p-3">
          <p className="text-muted-foreground text-xs">
            Copy this key now — it is only shown once.
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
                if (await copyText(createdKey.key, "Cloud API key copied")) {
                  createMutation.reset();
                }
              }}
            >
              <Copy className="size-3.5" />
              Copy
            </Button>
          </div>
        </div>
      )}

      <ul className="mt-3 flex flex-col gap-1.5">
        {(keysQuery.data ?? []).map((key) => (
          <CloudApiKeyRow
            key={key.id}
            apiKey={key}
            onRevoke={() => revokeMutation.mutate(key.id)}
          />
        ))}
        {keysQuery.data?.length === 0 && !createdKey && (
          <li className="text-muted-foreground text-xs">
            No cloud keys yet. Create one for a connector or remote agent.
          </li>
        )}
      </ul>
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
