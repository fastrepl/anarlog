import { Wrench } from "@phosphor-icons/react";
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
    <section className="flex flex-col gap-3">
      <h2 className="text-muted-foreground text-sm font-medium">Devtools</h2>
      <div className="border-border bg-card overflow-hidden rounded-2xl border">
        <div className="flex flex-col gap-4 p-4 sm:flex-row sm:items-start sm:justify-between">
          <div className="flex min-w-0 gap-3">
            <div className="bg-muted flex size-10 shrink-0 items-center justify-center rounded-xl">
              <Wrench className="size-5" />
            </div>
            <div className="min-w-0">
              <h3 className="font-medium">Devtools panel</h3>
              <p className="text-muted-foreground mt-1 text-sm leading-5">
                Preview notifications, toasts, updates, and billing dialogs.
                Only available in dev and staging builds.
              </p>
            </div>
          </div>
          <div className="flex shrink-0 items-center">
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={openMutation.isPending}
              onClick={() => openMutation.mutate()}
            >
              Open panel
            </Button>
          </div>
        </div>
      </div>
    </section>
  );
}
