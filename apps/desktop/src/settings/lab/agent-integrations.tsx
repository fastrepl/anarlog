import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CheckCircle2Icon,
  RefreshCwIcon,
  Trash2Icon,
  WrenchIcon,
} from "lucide-react";

import {
  type ProviderHealth,
  type ProviderKind,
  commands as agentCommands,
} from "@hypr/plugin-agent";
import { Button } from "@hypr/ui/components/ui/button";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { cn } from "@hypr/utils";

function providerLabel(provider: ProviderKind) {
  switch (provider) {
    case "claude":
      return "Claude Code";
    case "codex":
      return "Codex";
    case "opencode":
      return "OpenCode";
  }
}

function AgentIntegrationRow({
  provider,
  onUpdated,
}: {
  provider: ProviderHealth;
  onUpdated: () => Promise<void>;
}) {
  const integrationMutation = useMutation({
    mutationFn: async () => {
      const result = provider.integrationInstalled
        ? await agentCommands.uninstallCli({
            provider: provider.provider,
          })
        : await agentCommands.installCli({
            provider: provider.provider,
          });
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
    onSuccess: async (data) => {
      sonnerToast.success(data.message);
      await onUpdated();
    },
    onError: (error) => {
      sonnerToast.error(error.message);
    },
  });

  return (
    <div className="flex items-center justify-between border-b border-neutral-100 py-3 last:border-b-0">
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium">
            {providerLabel(provider.provider)}
          </span>
          {provider.version && (
            <span className="text-xs text-neutral-400">{provider.version}</span>
          )}
          {provider.integrationInstalled ? (
            <span
              className={cn([
                "flex items-center gap-1 rounded-full px-1.5 py-0.5",
                "bg-emerald-50 text-[10px] font-medium text-emerald-700",
              ])}
            >
              <CheckCircle2Icon className="h-3 w-3" />
              Hook active
            </span>
          ) : (
            <span
              className={cn([
                "rounded-full px-1.5 py-0.5",
                "bg-neutral-100 text-[10px] font-medium text-neutral-500",
              ])}
            >
              Hook not installed
            </span>
          )}
        </div>
        {provider.authStatus !== "unknown" && (
          <p className="text-xs text-neutral-500">
            {provider.authStatus === "authenticated"
              ? "Signed in"
              : "Not signed in"}
          </p>
        )}
      </div>
      <Button
        size="sm"
        variant={provider.integrationInstalled ? "destructive" : "outline"}
        onClick={() => integrationMutation.mutate()}
        disabled={integrationMutation.isPending}
        type="button"
        className="shrink-0"
      >
        {provider.integrationInstalled ? (
          <Trash2Icon className="mr-2 h-4 w-4" />
        ) : (
          <WrenchIcon className="mr-2 h-4 w-4" />
        )}
        {integrationMutation.isPending
          ? provider.integrationInstalled
            ? "Uninstalling..."
            : "Installing..."
          : provider.integrationInstalled
            ? "Uninstall"
            : "Install"}
      </Button>
    </div>
  );
}

export function AgentIntegrations() {
  const queryClient = useQueryClient();
  const healthQuery = useQuery({
    queryKey: ["agent-integrations-health"],
    queryFn: async () => {
      const result = await agentCommands.healthCheck();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data.providers;
    },
    refetchInterval: 30_000,
  });

  const refresh = async () => {
    await queryClient.invalidateQueries({
      queryKey: ["agent-integrations-health"],
    });
  };

  const installedProviders = (healthQuery.data ?? []).filter(
    (p) => p.installed,
  );

  return (
    <section className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-4">
        <div className="flex-1">
          <h3 className="mb-1 text-sm font-medium">Agent Integrations</h3>
          <p className="text-xs text-neutral-600">
            Install Char hooks into your coding agents.
          </p>
        </div>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => void refresh()}
          disabled={healthQuery.isFetching}
        >
          <RefreshCwIcon
            className={cn([
              "h-4 w-4",
              healthQuery.isFetching && "animate-spin",
            ])}
          />
        </Button>
      </div>

      {healthQuery.isError ? (
        <p className="text-xs text-red-600">
          Failed to load agent integration status.
        </p>
      ) : !healthQuery.isPending && installedProviders.length === 0 ? (
        <p className="text-xs text-neutral-400">
          No supported CLIs detected. Install Claude Code, Codex, or OpenCode to
          get started.
        </p>
      ) : (
        <div>
          {installedProviders.map((provider) => (
            <AgentIntegrationRow
              key={provider.provider}
              provider={provider}
              onUpdated={refresh}
            />
          ))}
        </div>
      )}
    </section>
  );
}
