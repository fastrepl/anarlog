import { Trans } from "@lingui/react/macro";
import { useMutation, useQuery } from "@tanstack/react-query";

import { Button } from "@anlg/ui/components/ui/button";

import { SettingRow } from "~/settings/setting-row";
import { commands } from "~/types/tauri.gen";

// Incremental Tauri -> GPUI migration: opt-in toggle, only on builds that
// ship the native shell next to this binary.
export function NativeShellRow() {
  const available = useQuery({
    queryKey: ["native-shell-available"],
    queryFn: () => commands.isNativeShellAvailable(),
    staleTime: Infinity,
  });

  const switchShell = useMutation({
    mutationFn: async () => {
      const result = await commands.switchToNativeShell();
      if (result.status === "error") {
        throw new Error(result.error);
      }
    },
  });

  if (!available.data) {
    return null;
  }

  return (
    <SettingRow
      title={<Trans>Try the new Anarlog (beta)</Trans>}
      description={
        switchShell.isError ? (
          <span className="text-destructive">{switchShell.error.message}</span>
        ) : (
          <Trans>
            A faster native interface built on GPUI. You can switch back from
            its settings at any time. Anarlog relaunches immediately.
          </Trans>
        )
      }
      controlWidth="content"
    >
      {(labelProps) => (
        <Button
          {...labelProps}
          variant="outline"
          size="sm"
          disabled={switchShell.isPending}
          onClick={() => switchShell.mutate()}
        >
          <Trans>Switch</Trans>
        </Button>
      )}
    </SettingRow>
  );
}
