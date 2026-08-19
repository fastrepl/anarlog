import { t } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import { useMutation, useQuery } from "@tanstack/react-query";

import { commands as windowsCommands } from "@anlg/plugin-windows";
import { Button } from "@anlg/ui/components/ui/button";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { commands } from "~/types/tauri.gen";

export function DevtoolsSection() {
  const enabledQuery = useQuery({
    queryKey: ["devtools-panel", "enabled"],
    queryFn: commands.showDevtool,
    staleTime: Infinity,
  });
  const openMutation = useMutation({
    mutationFn: async () => {
      const result = await windowsCommands.devtoolsPanelShow();
      if (result.status === "error") {
        throw new Error(result.error);
      }
    },
    onError: (error) => sonnerToast.error(error.message),
  });

  if (enabledQuery.data !== true) {
    return null;
  }

  return (
    <section className="flex flex-col gap-4">
      <h2 className="font-sans text-lg font-semibold">{t`Devtools`}</h2>
      <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
        <div className="min-w-0">
          <h3 className="text-sm font-medium">{t`Devtools panel`}</h3>
          <p className="text-muted-foreground mt-1 text-sm leading-5">
            <Trans>
              Preview notifications, toasts, updates, and billing dialogs. Only
              available in dev and staging builds.
            </Trans>
          </p>
        </div>
        <div className="flex shrink-0 items-center">
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={openMutation.isPending}
            onClick={() => openMutation.mutate()}
          >
            {t`Open panel`}
          </Button>
        </div>
      </div>
    </section>
  );
}
