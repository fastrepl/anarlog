import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open as selectFolder } from "@tauri-apps/plugin-dialog";
import { FolderIcon } from "lucide-react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as settingsCommands } from "@hypr/plugin-settings";

export function FolderLocationSection({
  onContinue,
}: {
  onContinue: () => void;
}) {
  const queryClient = useQueryClient();

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
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["vault-base-path"] });
    },
  });

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

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-3 rounded-lg border border-neutral-200 bg-neutral-50 px-4 py-3">
        <FolderIcon className="size-4 shrink-0 text-neutral-500" />
        <button
          onClick={handleOpenPath}
          className="min-w-0 flex-1 truncate text-left text-sm text-neutral-600 hover:underline"
        >
          {vaultBase ?? "Loading..."}
        </button>
        <button
          onClick={handleChange}
          disabled={changeMutation.isPending}
          className="shrink-0 text-sm text-neutral-500 transition-colors hover:text-neutral-700"
        >
          Change
        </button>
        <button
          onClick={onContinue}
          className="shrink-0 rounded-full bg-stone-600 px-3 py-1 text-sm font-medium text-white duration-150 hover:scale-[1.01] active:scale-[0.99]"
        >
          Confirm
        </button>
      </div>
    </div>
  );
}
