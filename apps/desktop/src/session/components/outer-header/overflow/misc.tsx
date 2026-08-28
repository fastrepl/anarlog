import { CircleNotch, FolderOpen } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import { platform } from "@tauri-apps/plugin-os";

import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { DropdownMenuItem } from "@anlg/ui/components/ui/dropdown-menu";

export function ShowInFolder({ sessionId }: { sessionId: string }) {
  const label = platform() === "macos" ? "Show in Finder" : "Show in folder";
  const { mutate, isPending } = useMutation({
    mutationFn: async () => {
      const result = await fsSyncCommands.sessionDir(sessionId);
      if (result.status === "error") {
        throw new Error(result.error);
      }
      await openerCommands.openPath(result.data, null);
    },
  });

  return (
    <DropdownMenuItem
      onClick={(e) => {
        e.preventDefault();
        mutate();
      }}
      disabled={isPending}
      sx={styles.item}
    >
      {isPending ? (
        <CircleNotch {...stylex.props(styles.spinner)} />
      ) : (
        <FolderOpen data-testid="show-in-folder-icon" />
      )}
      <span>{isPending ? "Opening..." : label}</span>
    </DropdownMenuItem>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  item: {
    cursor: "pointer",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
});
