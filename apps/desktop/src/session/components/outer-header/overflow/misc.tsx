import { useMutation } from "@tanstack/react-query";
import { platform } from "@tauri-apps/plugin-os";
import { FolderOpenIcon, Loader2Icon } from "lucide-react";

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
      className="cursor-pointer"
    >
      {isPending ? (
        <Loader2Icon className="animate-spin" />
      ) : (
        <FolderOpenIcon />
      )}
      <span>{isPending ? "Opening..." : label}</span>
    </DropdownMenuItem>
  );
}
