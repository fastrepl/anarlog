import {
  ArrowSquareOut,
  CheckCircle,
  CircleNotch,
  Code,
  Copy,
  Terminal,
} from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn } from "@anlg/utils";

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

async function copyText(
  text: string,
  successMessage: string,
  fallbackErrorMessage: string,
) {
  try {
    await navigator.clipboard.writeText(text);
    sonnerToast.success(successMessage);
  } catch (error) {
    sonnerToast.error(
      error instanceof Error ? error.message : fallbackErrorMessage,
    );
  }
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

export function CliSettingsSections() {
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
    <CliSection
      status={status}
      isLoading={statusQuery.isPending}
      error={statusQuery.error}
      isInstalling={installMutation.isPending}
      onInstall={() => installMutation.mutate()}
    />
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
      <h2 className="text-muted-foreground text-sm font-medium">CLI & MCP</h2>
      <div className="border-border bg-card divide-border divide-y overflow-hidden rounded-2xl border">
        <div className="flex flex-col gap-4 p-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex min-w-0 gap-3">
            <div className="bg-muted flex size-10 shrink-0 items-center justify-center rounded-xl">
              <Terminal className="size-5" />
            </div>
            <div className="min-w-0">
              <h3 className="font-medium">Anarlog CLI</h3>
              <CopyableCommand
                command={`${commandName} --json meetings list`}
              />
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
              <ArrowSquareOut className="size-3.5" />
            </Button>
            <Button
              type="button"
              size="sm"
              disabled={!canInstall || isInstalling}
              onClick={onInstall}
            >
              {isInstalling ? (
                <CircleNotch className="size-3.5 animate-spin" />
              ) : isInstalled ? (
                "Reinstall"
              ) : (
                "Install"
              )}
            </Button>
          </div>
        </div>
        <McpRow status={status} />
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
      <span className="text-muted-foreground mt-1 flex items-center gap-1.5 text-xs">
        <CircleNotch className="size-3 animate-spin" />
        Checking…
      </span>
    );
  }

  if (error) {
    return (
      <p className="text-destructive mt-1 text-xs">
        Could not check the CLI: {error.message}
      </p>
    );
  }

  if (!status) {
    return null;
  }

  const isInstalled = status.state === "installed";
  const showDetails = ["conflict", "unsupported", "resource_missing"].includes(
    status.state,
  );

  return (
    <div className="mt-1 flex items-start gap-1.5 text-xs">
      {isInstalled ? (
        <CheckCircle className="mt-0.5 size-3.5 shrink-0 text-emerald-600" />
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
      <span className="text-muted-foreground break-all">
        {isInstalled
          ? "Installed"
          : showDetails
            ? (status.details ?? "Unavailable")
            : "Not installed"}
      </span>
    </div>
  );
}

function CopyableCommand({ command }: { command: string }) {
  return (
    <div className="mt-1 flex min-w-0 items-center gap-1">
      <code className="bg-muted scrollbar-hide min-w-0 overflow-x-auto rounded-md px-1.5 py-0.5 text-xs font-medium">
        {command}
      </code>
      <Button
        type="button"
        variant="ghost"
        size="icon"
        className="size-6 shrink-0"
        aria-label={`Copy ${command}`}
        onClick={() =>
          void copyText(command, "Command copied", "Could not copy the command")
        }
      >
        <Copy className="size-3.5" />
      </Button>
    </div>
  );
}

function McpRow({ status }: { status: EmbeddedCliStatus | undefined }) {
  const isInstalled = status?.state === "installed";
  const commandName = status?.commandName ?? "anarlog";
  const configuration = buildMcpConfiguration(
    isInstalled ? status.installPath : commandName,
  );

  return (
    <div className="flex flex-col gap-4 p-4 sm:flex-row sm:items-center sm:justify-between">
      <div className="flex min-w-0 gap-3">
        <div className="bg-muted flex size-10 shrink-0 items-center justify-center rounded-xl">
          <Code className="size-5" />
        </div>
        <div className="min-w-0">
          <h3 className="font-medium">MCP server</h3>
          <CopyableCommand command={`${commandName} mcp`} />
        </div>
      </div>
      <div className="flex shrink-0 gap-2">
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => void openerCommands.openUrl(MCP_GUIDE_URL, null)}
        >
          Guide
          <ArrowSquareOut className="size-3.5" />
        </Button>
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={!isInstalled}
          onClick={() =>
            void copyText(
              configuration,
              "MCP configuration copied",
              "Could not copy the MCP configuration",
            )
          }
        >
          <Copy className="size-3.5" />
          Copy config
        </Button>
      </div>
    </div>
  );
}
