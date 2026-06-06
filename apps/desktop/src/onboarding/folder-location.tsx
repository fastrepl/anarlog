import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { homeDir } from "@tauri-apps/api/path";
import { message, open as selectFolder } from "@tauri-apps/plugin-dialog";
import { FolderIcon } from "lucide-react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as settingsCommands } from "@hypr/plugin-settings";

import { ObsidianVaultList } from "~/settings/general/storage/obsidian-vault-list";
import { displayPath } from "~/settings/general/storage/path-utils";
import { scheduleAutomaticRelaunch } from "~/store/tinybase/store/save";

async function handleStorageUpdate() {
  const restartStatus = await scheduleAutomaticRelaunch();

  if (restartStatus === "deferred") {
    void message(
      "The app will restart after onboarding to apply your storage changes",
      {
        kind: "info",
        title: "Storage Updated",
      },
    );
  }
}

export function FolderLocationSection({
  onContinue,
}: {
  onContinue: () => void;
}) {
  const queryClient = useQueryClient();

  const { data: home } = useQuery({ queryKey: ["home-dir"], queryFn: homeDir });
  const { data: vaultBase } = useQuery({
    queryKey: ["vault-base-path"],
    queryFn: async () => {
      const result = await settingsCommands.vaultBase();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  const { data: obsidianVaults } = useQuery({
    queryKey: ["obsidian-vaults"],
    queryFn: async () => {
      const result = await settingsCommands.obsidianVaults();
      if (result.status === "error") return [];
      return result.data;
    },
  });

  const changeMutation = useMutation({
    mutationFn: async (newPath: string) => {
      const copyResult = await settingsCommands.copyVault(newPath);
      if (copyResult.status === "error") {
        throw new Error(copyResult.error);
      }

      const result = await settingsCommands.setVaultBase(newPath);
      if (result.status === "error") {
        throw new Error(result.error);
      }
    },
    onSuccess: async () => {
      queryClient.invalidateQueries({ queryKey: ["vault-base-path"] });
      await handleStorageUpdate();
    },
  });

  const useObsidianVaultMutation = useMutation({
    mutationFn: async (vaultPath: string) => {
      const result = await settingsCommands.setVaultBase(vaultPath);
      if (result.status === "error") {
        throw new Error(result.error);
      }
    },
    onSuccess: async () => {
      queryClient.invalidateQueries({ queryKey: ["vault-base-path"] });
      await handleStorageUpdate();
    },
  });

  const isPending =
    changeMutation.isPending || useObsidianVaultMutation.isPending;

  const handleChange = async () => {
    const selected = await selectFolder({
      title: "Choose storage location",
      directory: true,
      multiple: false,
      defaultPath: vaultBase ?? undefined,
    });

    if (selected) {
      changeMutation.mutate(selected);
    }
  };

  const handleOpenPath = () => {
    if (vaultBase) {
      openerCommands.openPath(vaultBase, null);
    }
  };

  const detectedVaults = (obsidianVaults ?? []).filter(
    (v) => v.path !== vaultBase,
  );

  return (
    <div className="flex flex-col gap-3">
      <div className="border-border bg-muted flex items-center gap-3 rounded-lg border px-4 py-3">
        <FolderIcon className="text-muted-foreground size-4 shrink-0" />
        <button
          onClick={handleOpenPath}
          className="text-muted-foreground min-w-0 flex-1 truncate text-left text-sm hover:underline"
        >
          {displayPath(vaultBase, home)}
        </button>
        <button
          onClick={handleChange}
          disabled={isPending}
          className="text-muted-foreground hover:text-muted-foreground shrink-0 text-sm transition-colors disabled:opacity-50"
        >
          Change
        </button>
        <button
          onClick={onContinue}
          disabled={isPending}
          className="bg-primary text-primary-foreground hover:bg-primary/90 shrink-0 rounded-full px-3 py-1 text-sm font-medium duration-150 hover:scale-[1.01] active:scale-[0.99] disabled:opacity-50"
        >
          Confirm
        </button>
      </div>

      <ObsidianVaultList
        vaults={detectedVaults}
        home={home}
        disabled={isPending}
        onSelect={(path) => useObsidianVaultMutation.mutate(path)}
      />
    </div>
  );
}
