import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation } from "@tanstack/react-query";
import { relaunch } from "@tauri-apps/plugin-process";

import { toast } from "@anlg/ui/components/ui/toast";

import { DestructiveConfirmationDialog } from "~/shared/ui/destructive-confirmation-dialog";
import { commands } from "~/types/tauri.gen";

// The database belongs to another account. Starting fresh leaves a marker and
// relaunches; the next start moves the database aside as a backup and opens a
// new one, so this account can sync into an empty workspace.
export function StartFreshDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useLingui();
  const startFresh = useMutation({
    mutationFn: async () => {
      const result = await commands.requestLocalDatabaseReset();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      await relaunch();
    },
    onError: (error) => {
      toast.error(t`Could not start fresh: ${error.message}`);
    },
  });

  return (
    <DestructiveConfirmationDialog
      open={open}
      onOpenChange={onOpenChange}
      title={<Trans>Start fresh on this device?</Trans>}
      description={
        <Trans>
          The notes on this device belong to another Anarlog account. Starting
          fresh sets them aside as a backup file and restarts Anarlog with an
          empty workspace for the account you signed in with. Nothing from the
          old notes is uploaded.
        </Trans>
      }
      confirmLabel={<Trans>Start fresh</Trans>}
      pendingLabel={<Trans>Restarting…</Trans>}
      isPending={startFresh.isPending}
      onConfirm={() => startFresh.mutate()}
    />
  );
}
