import { useMutation } from "@tanstack/react-query";
import { open as selectFolder } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";

import { commands as settingsCommands } from "@hypr/plugin-settings";

import { scheduleAutomaticRelaunch } from "~/store/tinybase/store/save";

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
  const [moveVault, setMoveVault] = useState(true);

  const selectPath = (path: string | null) => {
    setSelectedPath(path);
    setMoveVault(true);
  };

  useEffect(() => {
    if (!open) return;
    setSelectedPath(currentPath ?? null);
    setMoveVault(true);
  }, [currentPath, open]);

  const applyMutation = useMutation({
    mutationFn: async ({
      newPath,
      shouldMove,
    }: {
      newPath: string;
      shouldMove: boolean;
    }) => {
      if (shouldMove) {
        const moveResult = await settingsCommands.moveVault(newPath);
        if (moveResult.status === "error") {
          throw new Error(moveResult.error);
        }
      } else {
        const setResult = await settingsCommands.setVaultBase(newPath);
        if (setResult.status === "error") {
          throw new Error(setResult.error);
        }
      }
    },
    onSuccess: async () => {
      onSuccess();
      await scheduleAutomaticRelaunch();
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
      selectPath(selected);
    }
  };

  return {
    selectedPath,
    selectPath,
    moveVault,
    setMoveVault,
    chooseFolder,
    apply: () => {
      if (selectedPath) {
        applyMutation.mutate({ newPath: selectedPath, shouldMove: moveVault });
      }
    },
    isPending: applyMutation.isPending,
    error: applyMutation.error,
  };
}
