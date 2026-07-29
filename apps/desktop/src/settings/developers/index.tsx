import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CheckCircle2Icon,
  Code2Icon,
  CopyIcon,
  ExternalLinkIcon,
  GlobeIcon,
  KeyRoundIcon,
  Loader2Icon,
  PlugIcon,
  TerminalIcon,
  Trash2Icon,
  WebhookIcon,
} from "lucide-react";

import {
  commands as localApiCommands,
  type ApiKeyInfo,
  type WebhookInfo,
} from "@anlg/plugin-local-api";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";
import { Switch } from "@anlg/ui/components/ui/switch";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn } from "@anlg/utils";

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
import { SettingsPageTitle } from "~/settings/page-title";
import { commands, type EmbeddedCliStatus } from "~/types/tauri.gen";

const CLI_STATUS_QUERY_KEY = ["embedded-cli-status"] as const;
const CLI_GUIDE_URL = "https://docs.anarlog.so/agents/cli";
const MCP_GUIDE_URL = "https://docs.anarlog.so/agents/mcp";
const API_GUIDE_URL = "https://docs.anarlog.so/reference/api";
const CLOUD_API_GUIDE_URL = "https://docs.anarlog.so/reference/api-cloud";
const CLOUD_API_BASE_URL = new URL("/v1", env.VITE_API_URL).toString();
const CLOUD_MCP_URL = new URL("/mcp", env.VITE_API_URL).toString();
const API_STATUS_QUERY_KEY = ["local-api", "status"] as const;
const API_KEYS_QUERY_KEY = ["local-api", "keys"] as const;
const API_WEBHOOKS_QUERY_KEY = ["local-api", "webhooks"] as const;
const CLOUD_API_SETTINGS_QUERY_KEY = ["cloud-api", "settings"] as const;
const CLOUD_API_KEYS_QUERY_KEY = ["cloud-api", "keys"] as const;

async function unwrap<T>(
  promise: Promise<
    { status: "ok"; data: T } | { status: "error"; error: string }
  >,
): Promise<T> {
  const result = await promise;
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

async function loadStatus() {
  const result = await commands.checkEmbeddedCli();
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

export function buildMcpConfiguration(command: string) {
  return JSON.stringify(
    {
      mcpServers: {
        anarlog: {
          command,
          args: ["mcp"],
        },
      },
    },
    null,
    2,
  );
}

export function getCliInstallNotification(status: EmbeddedCliStatus) {
  if (status.state === "installed") {
    return {
      type: "success" as const,
      message: `${status.commandName} is ready to use`,
    };
  }

  return {
    type: "error" as const,
    message: status.details ?? `${status.commandName} could not be installed`,
  };
}

export function SettingsDevelopers() {
  const queryClient = useQueryClient();
  const statusQuery = useQuery({
    queryKey: CLI_STATUS_QUERY_KEY,
    queryFn: loadStatus,
  });
  const installMutation = useMutation({
    mutationFn: async () => {
      const result = await commands.installEmbeddedCli();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
    onSuccess: (status) => {
      queryClient.setQueryData(CLI_STATUS_QUERY_KEY, status);
      const notification = getCliInstallNotification(status);
      if (notification.type === "success") {
        sonnerToast.success(notification.message);
      } else {
        sonnerToast.error(notification.message);
      }
    },
    onError: (error) => sonnerToast.error(error.message),
  });

  const status = statusQuery.data;

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-8">
      <SettingsPageTitle title="Developers" />
      <CliSection
        status={status}
        isLoading={statusQuery.isPending}
        error={statusQuery.error}
        isInstalling={installMutation.isPending}
        onInstall={() => installMutation.mutate()}
      />
      <McpSection status={status} />
      <CloudApiSection />
      <ApiSection />
    </div>
  );
}

function CliSection({
  status,
  isLoading,
  error,
  isInstalling,
  onInstall,
}: {
  status: EmbeddedCliStatus | undefined;
  isLoading: boolean;
  error: Error | null;
  isInstalling: boolean;
  onInstall: () => void;
}) {
  const commandName = status?.commandName ?? "anarlog";
  const canInstall =
    status?.supported === true &&
    status.state !== "resource_missing" &&
    status.state !== "conflict";
  const isInstalled = status?.state === "installed";

  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-muted-foreground text-sm font-medium">CLI</h2>
      <div className="border-border bg-card overflow-hidden rounded-2xl border">
        <div className="flex flex-col gap-4 p-4 sm:flex-row sm:items-start sm:justify-between">
          <div className="flex min-w-0 gap-3">
            <div className="bg-muted flex size-10 shrink-0 items-center justify-center rounded-xl">
              <TerminalIcon className="size-5" />
            </div>
            <div className="min-w-0">
              <h3 className="font-medium">Anarlog CLI</h3>
              <p className="text-muted-foreground mt-1 text-sm leading-5">
                Browse notes, summaries, transcripts, and recurring meetings
                from the command line. The MCP server is included.
              </p>
              <CliStatus status={status} isLoading={isLoading} error={error} />
            </div>
          </div>
          <div className="flex shrink-0 gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => void openerCommands.openUrl(CLI_GUIDE_URL, null)}
            >
              Guide
              <ExternalLinkIcon className="size-3.5" />
            </Button>
            <Button
              type="button"
              size="sm"
              disabled={!canInstall || isInstalling}
              onClick={onInstall}
            >
              {isInstalling ? (
                <Loader2Icon className="size-3.5 animate-spin" />
              ) : isInstalled ? (
                "Reinstall"
              ) : (
                "Install"
              )}
            </Button>
          </div>
        </div>

        <div className="border-border divide-border divide-y border-t">
          <CommandExample
            icon={<TerminalIcon className="size-4" />}
            command={`${commandName} --json meetings list`}
            description="List recent meetings for scripts and coding agents."
          />
          <CommandExample
            icon={<PlugIcon className="size-4" />}
            command={`${commandName} mcp`}
            description="Connect local Anarlog meeting context over MCP."
          />
        </div>
      </div>
    </section>
  );
}

function CliStatus({
  status,
  isLoading,
  error,
}: {
  status: EmbeddedCliStatus | undefined;
  isLoading: boolean;
  error: Error | null;
}) {
  if (isLoading) {
    return (
      <span className="text-muted-foreground mt-2 flex items-center gap-1.5 text-xs">
        <Loader2Icon className="size-3 animate-spin" />
        Checking installation…
      </span>
    );
  }

  if (error) {
    return (
      <p className="text-destructive mt-2 text-xs">
        Could not check the CLI: {error.message}
      </p>
    );
  }

  if (!status) {
    return null;
  }

  return (
    <div className="mt-2 flex items-start gap-1.5 text-xs">
      {status.state === "installed" ? (
        <CheckCircle2Icon className="mt-0.5 size-3.5 shrink-0 text-emerald-600" />
      ) : (
        <span
          className={cn([
            "mt-1 size-2 shrink-0 rounded-full",
            status.state === "conflict"
              ? "bg-amber-500"
              : "bg-muted-foreground/50",
          ])}
        />
      )}
      <span className="text-muted-foreground break-all">{status.details}</span>
    </div>
  );
}

function CommandExample({
  icon,
  command,
  description,
}: {
  icon: React.ReactNode;
  command: string;
  description: string;
}) {
  return (
    <div className="flex items-start gap-3 px-4 py-3">
      <span className="text-muted-foreground mt-0.5 shrink-0">{icon}</span>
      <div className="min-w-0">
        <code className="bg-muted rounded-md px-1.5 py-0.5 text-xs font-medium">
          {command}
        </code>
        <p className="text-muted-foreground mt-1 text-xs">{description}</p>
      </div>
    </div>
  );
}

function McpSection({ status }: { status: EmbeddedCliStatus | undefined }) {
  const isInstalled = status?.state === "installed";
  const command = isInstalled
    ? status.installPath
    : (status?.commandName ?? "anarlog");
  const configuration = buildMcpConfiguration(command);

  const copyConfiguration = async () => {
    try {
      await navigator.clipboard.writeText(configuration);
      sonnerToast.success("MCP configuration copied");
    } catch (error) {
      sonnerToast.error(
        error instanceof Error
          ? error.message
          : "Could not copy the MCP configuration",
      );
    }
  };

  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-muted-foreground text-sm font-medium">MCP</h2>
      <div className="border-border bg-card overflow-hidden rounded-2xl border">
        <div className="flex items-start justify-between gap-4 p-4">
          <div className="flex min-w-0 gap-3">
            <div className="bg-muted flex size-10 shrink-0 items-center justify-center rounded-xl">
              <Code2Icon className="size-5" />
            </div>
            <div>
              <h3 className="font-medium">Anarlog MCP server</h3>
              <p className="text-muted-foreground mt-1 text-sm leading-5">
                Add read-only local meeting context to agents that support MCP.
              </p>
            </div>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="shrink-0"
            onClick={() => void openerCommands.openUrl(MCP_GUIDE_URL, null)}
          >
            Guide
            <ExternalLinkIcon className="size-3.5" />
          </Button>
        </div>

        <div className="border-border bg-muted/30 border-t p-3">
          <div className="mb-2 flex items-center justify-between gap-3">
            <span className="text-muted-foreground text-xs font-medium">
              mcp.json
            </span>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-7"
              disabled={!isInstalled}
              onClick={() => void copyConfiguration()}
            >
              <CopyIcon className="size-3.5" />
              Copy
            </Button>
          </div>
          <pre className="scrollbar-hide overflow-x-auto text-xs leading-5">
            <code>{configuration}</code>
          </pre>
        </div>
      </div>
    </section>
  );
}

async function copyText(value: string, message: string) {
  try {
    await navigator.clipboard.writeText(value);
    sonnerToast.success(message);
    return true;
  } catch (error) {
    sonnerToast.error(
      error instanceof Error ? error.message : "Could not copy to clipboard",
    );
    return false;
  }
}

function CloudApiSection() {
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
              <GlobeIcon className="size-5" />
            </div>
            <div className="min-w-0">
              <h3 className="font-medium">Hosted access for agents</h3>
              <p className="text-muted-foreground mt-1 text-sm leading-5">
                Access your meetings through the hosted REST API and remote MCP
                even when Anarlog is not running.
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
              <ExternalLinkIcon className="size-3.5" />
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
          <CopyIcon className="size-3.5" />
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
        <KeyRoundIcon className="text-muted-foreground size-4" />
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
              <CopyIcon className="size-3.5" />
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

function ApiSection() {
  const queryClient = useQueryClient();

  const statusQuery = useQuery({
    queryKey: API_STATUS_QUERY_KEY,
    queryFn: () => unwrap(localApiCommands.getStatus()),
  });
  const toggleMutation = useMutation({
    mutationFn: (enabled: boolean) =>
      unwrap(localApiCommands.setEnabled(enabled)),
    onSuccess: (status) => {
      queryClient.setQueryData(API_STATUS_QUERY_KEY, status);
    },
    onError: (error) => sonnerToast.error(error.message),
  });

  const status = statusQuery.data;
  const running = status?.running === true;

  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-muted-foreground text-sm font-medium">Local API</h2>
      <div className="border-border bg-card overflow-hidden rounded-2xl border">
        <div className="flex flex-col gap-4 p-4 sm:flex-row sm:items-start sm:justify-between">
          <div className="flex min-w-0 gap-3">
            <div className="bg-muted flex size-10 shrink-0 items-center justify-center rounded-xl">
              <GlobeIcon className="size-5" />
            </div>
            <div className="min-w-0">
              <h3 className="font-medium">Anarlog local API</h3>
              <p className="text-muted-foreground mt-1 text-sm leading-5">
                Serve your meetings, notes, and transcripts over a local REST
                API with API keys and outbound webhooks. Data never leaves this
                device.
              </p>
              <p className="text-muted-foreground mt-2 flex items-center gap-1.5 text-xs">
                {running ? (
                  <>
                    <CheckCircle2Icon className="size-3.5 shrink-0 text-emerald-600" />
                    <code className="bg-muted rounded-md px-1.5 py-0.5">
                      http://127.0.0.1:{status?.port}
                    </code>
                  </>
                ) : (
                  <>
                    <span className="bg-muted-foreground/50 size-2 shrink-0 rounded-full" />
                    Server is off
                  </>
                )}
              </p>
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => void openerCommands.openUrl(API_GUIDE_URL, null)}
            >
              Guide
              <ExternalLinkIcon className="size-3.5" />
            </Button>
            <Switch
              checked={status?.enabled === true}
              disabled={statusQuery.isPending || toggleMutation.isPending}
              onCheckedChange={(checked) => toggleMutation.mutate(checked)}
            />
          </div>
        </div>

        <ApiKeysCard />
        <WebhooksCard />
      </div>
    </section>
  );
}

function ApiKeysCard() {
  const queryClient = useQueryClient();
  const keysQuery = useQuery({
    queryKey: API_KEYS_QUERY_KEY,
    queryFn: () => unwrap(localApiCommands.listApiKeys()),
  });
  const createMutation = useMutation({
    mutationFn: (name: string) => unwrap(localApiCommands.createApiKey(name)),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: API_KEYS_QUERY_KEY }),
    onError: (error) => sonnerToast.error(error.message),
  });
  const revokeMutation = useMutation({
    mutationFn: (id: string) => unwrap(localApiCommands.revokeApiKey(id)),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: API_KEYS_QUERY_KEY }),
    onError: (error) => sonnerToast.error(error.message),
  });

  const form = useForm({
    defaultValues: { name: "" },
    onSubmit: ({ value }) => {
      createMutation.mutate(value.name.trim() || "API key");
      form.setFieldValue("name", "");
    },
  });

  const createdKey = createMutation.data;

  return (
    <div className="border-border border-t p-4">
      <div className="mb-3 flex items-center gap-2">
        <KeyRoundIcon className="text-muted-foreground size-4" />
        <h4 className="text-sm font-medium">API keys</h4>
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
              placeholder="Key name (e.g. Zapier)"
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
                if (await copyText(createdKey.key, "API key copied")) {
                  createMutation.reset();
                }
              }}
            >
              <CopyIcon className="size-3.5" />
              Copy
            </Button>
          </div>
        </div>
      )}

      <ul className="mt-3 flex flex-col gap-1.5">
        {(keysQuery.data ?? []).map((key) => (
          <ApiKeyRow
            key={key.id}
            apiKey={key}
            onRevoke={() => revokeMutation.mutate(key.id)}
          />
        ))}
        {keysQuery.data?.length === 0 && !createdKey && (
          <li className="text-muted-foreground text-xs">
            No API keys yet. Create one to call the API.
          </li>
        )}
      </ul>
    </div>
  );
}

function ApiKeyRow({
  apiKey,
  onRevoke,
}: {
  apiKey: Pick<
    ApiKeyInfo,
    "id" | "name" | "key_prefix" | "created_at" | "last_used_at"
  >;
  onRevoke: () => void;
}) {
  return (
    <li className="flex items-center justify-between gap-3 text-sm">
      <div className="flex min-w-0 items-center gap-2">
        <code className="bg-muted shrink-0 rounded-md px-1.5 py-0.5 text-xs">
          {apiKey.key_prefix}…
        </code>
        <span className="truncate">{apiKey.name}</span>
      </div>
      <div className="flex shrink-0 items-center gap-2">
        <span className="text-muted-foreground text-xs">
          {apiKey.last_used_at
            ? `Last used ${apiKey.last_used_at.slice(0, 10)}`
            : `Created ${apiKey.created_at.slice(0, 10)}`}
        </span>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="text-destructive h-7"
          onClick={onRevoke}
        >
          Revoke
        </Button>
      </div>
    </li>
  );
}

function WebhooksCard() {
  const queryClient = useQueryClient();
  const webhooksQuery = useQuery({
    queryKey: API_WEBHOOKS_QUERY_KEY,
    queryFn: () => unwrap(localApiCommands.listWebhooks()),
  });
  const createMutation = useMutation({
    mutationFn: (url: string) =>
      unwrap(localApiCommands.createWebhook(url, [])),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: API_WEBHOOKS_QUERY_KEY }),
    onError: (error) => sonnerToast.error(error.message),
  });
  const deleteMutation = useMutation({
    mutationFn: (id: string) => unwrap(localApiCommands.deleteWebhook(id)),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: API_WEBHOOKS_QUERY_KEY }),
    onError: (error) => sonnerToast.error(error.message),
  });
  const testMutation = useMutation({
    mutationFn: (id: string) => unwrap(localApiCommands.testWebhook(id)),
    onSuccess: (delivery) => {
      if (delivery.delivered) {
        sonnerToast.success(`Test delivered (${delivery.status})`);
      } else {
        sonnerToast.error(`Test failed (${delivery.status})`);
      }
      void queryClient.invalidateQueries({ queryKey: API_WEBHOOKS_QUERY_KEY });
    },
    onError: (error) => sonnerToast.error(error.message),
  });

  const form = useForm({
    defaultValues: { url: "" },
    onSubmit: ({ value }) => {
      const url = value.url.trim();
      if (!url) {
        return;
      }
      createMutation.mutate(url);
      form.setFieldValue("url", "");
    },
  });

  const createdWebhook = createMutation.data;

  return (
    <div className="border-border border-t p-4">
      <div className="mb-3 flex items-center gap-2">
        <WebhookIcon className="text-muted-foreground size-4" />
        <h4 className="text-sm font-medium">Webhooks</h4>
      </div>
      <p className="text-muted-foreground mb-3 text-xs">
        Anarlog sends signed events when a meeting ends and when a summary is
        generated.
      </p>

      <form
        className="flex gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          event.stopPropagation();
          void form.handleSubmit();
        }}
      >
        <form.Field name="url">
          {(field) => (
            <Input
              className="h-8 max-w-md text-sm"
              placeholder="https://example.com/webhooks/anarlog"
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
          Add webhook
        </Button>
      </form>

      {createdWebhook && (
        <div className="border-border bg-muted/30 mt-3 rounded-xl border p-3">
          <p className="text-muted-foreground text-xs">
            Signing secret — copy it now, it is only shown once. Use it to
            verify the <code>x-anarlog-signature</code> header.
          </p>
          <div className="mt-2 flex items-center gap-2">
            <code className="bg-muted scrollbar-hide overflow-x-auto rounded-md px-1.5 py-0.5 text-xs">
              {createdWebhook.secret}
            </code>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-7 shrink-0"
              onClick={async () => {
                if (
                  await copyText(createdWebhook.secret, "Signing secret copied")
                ) {
                  createMutation.reset();
                }
              }}
            >
              <CopyIcon className="size-3.5" />
              Copy
            </Button>
          </div>
        </div>
      )}

      <ul className="mt-3 flex flex-col gap-1.5">
        {(webhooksQuery.data ?? []).map((webhook) => (
          <WebhookRow
            key={webhook.id}
            webhook={webhook}
            onTest={() => testMutation.mutate(webhook.id)}
            onDelete={() => deleteMutation.mutate(webhook.id)}
            isTesting={testMutation.isPending}
          />
        ))}
        {webhooksQuery.data?.length === 0 && !createdWebhook && (
          <li className="text-muted-foreground text-xs">
            No webhooks yet. Add an endpoint to receive events.
          </li>
        )}
      </ul>
    </div>
  );
}

function WebhookRow({
  webhook,
  onTest,
  onDelete,
  isTesting,
}: {
  webhook: WebhookInfo;
  onTest: () => void;
  onDelete: () => void;
  isTesting: boolean;
}) {
  return (
    <li className="flex items-center justify-between gap-3 text-sm">
      <div className="flex min-w-0 flex-col">
        <span className="truncate">{webhook.url}</span>
        <span className="text-muted-foreground text-xs">
          {webhook.events.length > 0 ? webhook.events.join(", ") : "All events"}
          {webhook.last_delivery_at &&
            ` · Last delivery ${webhook.last_delivery_status}`}
        </span>
      </div>
      <div className="flex shrink-0 items-center gap-1">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="h-7"
          disabled={isTesting}
          onClick={onTest}
        >
          Test
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="text-destructive h-7"
          onClick={onDelete}
        >
          <Trash2Icon className="size-3.5" />
        </Button>
      </div>
    </li>
  );
}
