import { useLingui } from "@lingui/react/macro";
import { FolderSimple } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { homeDir } from "@tauri-apps/api/path";
import { message, open as selectFolder } from "@tauri-apps/plugin-dialog";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { commands as settingsCommands } from "@anlg/plugin-settings";

import { ObsidianVaultList } from "~/settings/general/storage/obsidian-vault-list";
import { displayPath } from "~/settings/general/storage/path-utils";
import { scheduleAutomaticRelaunch } from "~/shared/relaunch";

export function FolderLocationSection({
  onContinue,
}: {
  onContinue: () => void;
}) {
  const { t } = useLingui();
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

  const handleStorageUpdate = async () => {
    const restartStatus = await scheduleAutomaticRelaunch();

    if (restartStatus === "deferred") {
      void message(
        t`The app will restart after onboarding to apply your storage changes`,
        {
          kind: "info",
          title: t`Storage Updated`,
        },
      );
    }
  };

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
      title: t`Choose storage location`,
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
    <div {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.location)}>
        <FolderSimple {...stylex.props(styles.folderIcon)} />
        <button onClick={handleOpenPath} {...stylex.props(styles.pathButton)}>
          {displayPath(vaultBase, home)}
        </button>
        <button
          onClick={handleChange}
          disabled={isPending}
          {...stylex.props(styles.changeButton)}
        >
          {t`Change`}
        </button>
        <button
          onClick={onContinue}
          disabled={isPending}
          {...stylex.props(styles.confirmButton)}
        >
          {t`Confirm`}
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

const styles = stylex.create({
  changeButton: {
    color: colors.mutedForeground,
    flexShrink: 0,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  confirmButton: {
    backgroundColor: {
      default: colors.primary,
      ":hover": `color-mix(in oklab, ${colors.primary} 90%, transparent)`,
    },
    borderRadius: radii.full,
    color: colors.primaryForeground,
    flexShrink: 0,
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    paddingBlock: "0.25rem",
    paddingInline: "0.75rem",
    transform: {
      default: "scale(1)",
      ":hover": "scale(1.01)",
      ":active": "scale(0.99)",
    },
    transitionDuration: "150ms",
  },
  folderIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  location: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    gap: "0.75rem",
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
  },
  pathButton: {
    color: colors.mutedForeground,
    flex: "1",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    minWidth: 0,
    overflow: "hidden",
    textAlign: "left",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    textDecorationLine: {
      default: "none",
      ":hover": "underline",
    },
  },
  root: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
  },
});
