import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CheckCircle2Icon,
  Code2Icon,
  CopyIcon,
  ExternalLinkIcon,
  Loader2Icon,
  PlugIcon,
  TerminalIcon,
} from "lucide-react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Button } from "@hypr/ui/components/ui/button";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { cn } from "@hypr/utils";

import { SettingsPageTitle } from "~/settings/page-title";
import { commands, type EmbeddedCliStatus } from "~/types/tauri.gen";

const CLI_STATUS_QUERY_KEY = ["embedded-cli-status"] as const;
const CLI_GUIDE_URL = "https://docs.anarlog.so/agents/cli";
const MCP_GUIDE_URL = "https://docs.anarlog.so/agents/mcp";

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
