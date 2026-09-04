import { Trans, useLingui } from "@lingui/react/macro";
import { CircleNotch, FolderSimple } from "@phosphor-icons/react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { homeDir } from "@tauri-apps/api/path";
import { open as selectFolder } from "@tauri-apps/plugin-dialog";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { commands as settingsCommands } from "@anlg/plugin-settings";
import { Button } from "@anlg/ui/components/ui/button";

import {
  LegacyMigrationCleanupRow,
  useLegacyMigrationCleanup,
} from "./legacy-cleanup";
import { displayPath } from "./path-utils";

import {
  flushApplicationState,
  scheduleAutomaticRelaunch,
} from "~/shared/relaunch";

function StorageLocationRow() {
  const { t } = useLingui();
  const { data: home } = useQuery({ queryKey: ["home-dir"], queryFn: homeDir });
  const vaultBaseQuery = useQuery({
    queryKey: ["vault-base-path"],
    queryFn: async () => {
      const result = await settingsCommands.vaultBase();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });
  const changeMutation = useMutation({
    mutationFn: async (newPath: string) => {
      await flushApplicationState();

      const moveResult = await settingsCommands.moveVault(newPath);
      if (moveResult.status === "error") {
        throw new Error(moveResult.error);
      }

      await scheduleAutomaticRelaunch();
    },
  });

  const handleChange = async () => {
    const selected = await selectFolder({
      title: t`Choose storage location`,
      directory: true,
      multiple: false,
      defaultPath: vaultBaseQuery.data,
    });

    if (selected && selected !== vaultBaseQuery.data) {
      changeMutation.mutate(selected);
    }
  };

  return (
    <div>
      <div className="grid grid-cols-[minmax(0,1fr)_9rem] items-center gap-3">
        <button
          type="button"
          className="hover:bg-muted/40 flex min-w-0 items-center gap-2 rounded-lg px-2 py-2 text-left transition-colors"
          disabled={!vaultBaseQuery.data}
          onClick={() => {
            if (vaultBaseQuery.data) {
              void openerCommands.openPath(vaultBaseQuery.data, null);
            }
          }}
        >
          <FolderSimple className="text-muted-foreground size-4 shrink-0" />
          <div className="min-w-0">
            <p className="text-sm font-medium">
              <Trans>Where your notes and recordings are stored</Trans>
            </p>
            <p className="text-muted-foreground truncate text-xs">
              {displayPath(vaultBaseQuery.data, home)}
            </p>
          </div>
        </button>
        <Button
          variant="outline"
          className="h-9 w-full justify-center"
          disabled={vaultBaseQuery.isPending || changeMutation.isPending}
          onClick={() => void handleChange()}
        >
          {changeMutation.isPending && (
            <CircleNotch className="size-4 animate-spin" aria-hidden="true" />
          )}
          <Trans>Change</Trans>
        </Button>
      </div>
      {(vaultBaseQuery.error || changeMutation.error) && (
        <p className="mt-1 text-xs text-red-500">
          {(vaultBaseQuery.error ?? changeMutation.error)?.message}
        </p>
      )}
    </div>
  );
}

export function StorageSettingsView() {
  const { visible } = useLegacyMigrationCleanup();

  return (
    <div>
      <h2 className="mb-4 font-sans text-lg font-semibold">
        <Trans>Storage</Trans>
      </h2>
      <div className="flex flex-col gap-3">
        <StorageLocationRow />
        {visible && <LegacyMigrationCleanupRow />}
      </div>
    </div>
  );
}
