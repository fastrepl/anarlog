import { useMutation, useQuery } from "@tanstack/react-query";
import { getIdentifier, getVersion } from "@tauri-apps/api/app";
import { useEffect } from "react";

import { commands as windowsCommands } from "@anlg/plugin-windows";
import { cn } from "@anlg/utils";

import { getDevtoolsChannel } from "~/shared/utils";
import { commands } from "~/types/tauri.gen";

export function EnvironmentModeBar() {
  const enabledQuery = useQuery({
    queryKey: ["devtools-panel", "enabled"],
    queryFn: commands.showDevtool,
    staleTime: Infinity,
  });
  const identityQuery = useQuery({
    queryKey: ["devtools-panel", "identity"],
    queryFn: loadDevtoolsIdentity,
    enabled: enabledQuery.data === true,
    staleTime: Infinity,
  });
  const openMutation = useMutation({
    mutationFn: async () => {
      const result = await windowsCommands.devtoolsPanelShow();
      if (result.status === "error") {
        throw new Error(result.error);
      }
    },
  });

  const identity = identityQuery.data;

  useEffect(() => {
    if (!identity) {
      return;
    }

    document.documentElement.dataset.devtoolsChannel = identity.channel;
    return () => {
      delete document.documentElement.dataset.devtoolsChannel;
    };
  }, [identity]);

  if (enabledQuery.data !== true || !identity) {
    return null;
  }

  const label = identity.channel === "staging" ? "STAGING" : "DEV";

  return (
    <div
      className={cn([
        "border-border bg-muted/80 text-muted-foreground",
        "flex h-8 shrink-0 items-center gap-3 border-t px-2.5",
      ])}
      data-channel={identity.channel}
      data-testid="environment-mode-bar"
    >
      <button
        type="button"
        aria-label={`Open Devtools panel (${label} v${identity.version})`}
        className={cn([
          "flex min-w-0 items-center gap-1.5 text-[11px] leading-none tracking-wide",
          "hover:text-foreground outline-hidden",
        ])}
        disabled={openMutation.isPending}
        onClick={() => openMutation.mutate()}
      >
        <span
          aria-hidden
          className={cn([
            "size-1.5 shrink-0 rounded-full",
            identity.channel === "staging" ? "bg-sky-500" : "bg-amber-500",
          ])}
        />
        <span className="text-foreground font-medium">{label}</span>
        <span>v{identity.version}</span>
      </button>
      <div className="min-w-0 flex-1" aria-hidden />
    </div>
  );
}

async function loadDevtoolsIdentity() {
  const [identifier, version] = await Promise.all([
    getIdentifier(),
    getVersion(),
  ]);

  return {
    channel: getDevtoolsChannel(identifier),
    version,
  };
}
