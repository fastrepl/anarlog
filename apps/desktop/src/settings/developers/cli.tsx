import { CheckCircle, CircleNotch, Copy } from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { Button } from "@anlg/ui/components/ui/button";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn } from "@anlg/utils";

import { commands, type EmbeddedCliStatus } from "~/types/tauri.gen";

const CLI_STATUS_QUERY_KEY = ["embedded-cli-status"] as const;

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
  const canInstall =
    status?.supported === true &&
    status.state !== "resource_missing" &&
    status.state !== "conflict";
  const isInstalled = status?.state === "installed";

  return (
    <section className="flex flex-col gap-4">
      <h2 className="font-sans text-lg font-semibold">CLI & MCP</h2>
      <div className="flex flex-col gap-4">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="min-w-0">
            <h3 className="flex items-center gap-1.5 text-sm font-medium">
              Anarlog CLI
              {isInstalled && (
                <CheckCircle
                  aria-label="Installed"
                  className="size-3.5 text-emerald-600"
                />
              )}
            </h3>
            <CliStatus status={status} isLoading={isLoading} error={error} />
          </div>
          <div className="flex shrink-0 gap-2">
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

  if (status.state === "installed") {
    return null;
  }

  const showDetails = ["conflict", "unsupported", "resource_missing"].includes(
    status.state,
  );

  return (
    <div className="mt-1 flex items-start gap-1.5 text-xs">
      <span
        className={cn([
          "mt-1 size-2 shrink-0 rounded-full",
          status.state === "conflict"
            ? "bg-amber-500"
            : "bg-muted-foreground/50",
        ])}
      />
      <span className="text-muted-foreground break-all">
        {showDetails ? (status.details ?? "Unavailable") : "Not installed"}
      </span>
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
    <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
      <div className="min-w-0">
        <h3 className="text-sm font-medium">MCP server</h3>
      </div>
      <div className="flex shrink-0 gap-2">
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
