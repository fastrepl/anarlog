import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import type { EmbeddedCliStatus } from "../../../shared/embedded-cli";

import { hypr } from "~/bridge";
import { CommandLineView } from "~/settings/command-line/command-line.view";

const QUERY_KEY = ["embedded-cli-status"] as const;

// Ported from `apps/desktop/src/settings/lab/command-line.tsx`. Behavior is
// identical; the only diff is the transport (`hypr.embeddedCli.*` instead of
// the Tauri generated `commands.checkEmbeddedCli` + friends).
export function CommandLineContainer() {
  const queryClient = useQueryClient();

  const query = useQuery<EmbeddedCliStatus>({
    queryKey: QUERY_KEY,
    queryFn: () => hypr.embeddedCli.check(),
  });

  const installMutation = useMutation({
    mutationFn: () => hypr.embeddedCli.install(),
    onSuccess: (data) => {
      queryClient.setQueryData<EmbeddedCliStatus>(QUERY_KEY, data);
    },
  });

  const uninstallMutation = useMutation({
    mutationFn: () => hypr.embeddedCli.uninstall(),
    onSuccess: (data) => {
      queryClient.setQueryData<EmbeddedCliStatus>(QUERY_KEY, data);
    },
  });

  return (
    <CommandLineView
      status={query.data}
      isLoading={query.isPending}
      isError={query.isError}
      isBusy={installMutation.isPending || uninstallMutation.isPending}
      installPending={installMutation.isPending}
      uninstallPending={uninstallMutation.isPending}
      errorMessage={
        installMutation.error?.message ??
        uninstallMutation.error?.message ??
        null
      }
      onInstall={() => installMutation.mutate()}
      onUninstall={() => uninstallMutation.mutate()}
    />
  );
}
