import { useMutation } from "@tanstack/react-query";
import { open as selectFolder } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";

import { commands as settingsCommands } from "@hypr/plugin-settings";

import { relaunch } from "~/store/tinybase/store/save";

export function useChangeContentPathWizard({
  open,
  currentPath,
  onSuccess,
}: {
  open: boolean;
  currentPath: string | undefined;
  onSuccess: () => void;
}) {
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [copyVault, setCopyVault] = useState(true);

  useEffect(() => {
    if (!open) return;
    setSelectedPath(currentPath ?? null);
    setCopyVault(true);
  }, [currentPath, open]);

  const applyMutation = useMutation({
    mutationFn: async ({
      newPath,
      shouldCopy,
    }: {
      newPath: string;
      shouldCopy: boolean;
    }) => {
      if (shouldCopy) {
        const copyResult = await settingsCommands.copyVault(newPath);
        if (copyResult.status === "error") {
          throw new Error(copyResult.error);
        }
      }

      const setResult = await settingsCommands.setVaultBase(newPath);
      if (setResult.status === "error") {
        throw new Error(setResult.error);
      }
    },
    onSuccess: async () => {
      onSuccess();
      await relaunch();
    },
  });

  const chooseFolder = async (defaultPath?: string) => {
    const selected = await selectFolder({
      title: "Choose content location",
      directory: true,
      multiple: false,
      defaultPath: defaultPath ?? selectedPath ?? undefined,
    });

    if (selected) {
      setSelectedPath(selected);
    }
  };

  return {
    selectedPath,
    selectPath: setSelectedPath,
    copyVault,
    setCopyVault,
    chooseFolder,
    apply: () => {
      if (selectedPath) {
        applyMutation.mutate({ newPath: selectedPath, shouldCopy: copyVault });
      }
    },
    isPending: applyMutation.isPending,
    error: applyMutation.error,
  };
}
