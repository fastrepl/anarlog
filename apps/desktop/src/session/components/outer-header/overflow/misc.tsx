import { Icon } from "@iconify-icon/react";
import { useMutation } from "@tanstack/react-query";
import { FolderIcon, Link2Icon, Loader2Icon } from "lucide-react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import {
  DropdownMenuItem,
  DropdownMenuSub,
  DropdownMenuSubTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";

import { SearchableFolderSubmenuContent } from "~/session/components/outer-header/shared/folder";

export function Copy() {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <DropdownMenuItem
          disabled={true}
          className="cursor-not-allowed"
          onSelect={(e) => e.preventDefault()}
        >
          <Link2Icon />
          <span>Copy link</span>
        </DropdownMenuItem>
      </TooltipTrigger>
      <TooltipContent side="left">
        <span>Coming soon</span>
      </TooltipContent>
    </Tooltip>
  );
}

export function Folder({
  sessionId,
  setOpen,
}: {
  sessionId: string;
  setOpen?: (open: boolean) => void;
}) {
  return (
    <DropdownMenuSub>
      <DropdownMenuSubTrigger className="cursor-pointer">
        <FolderIcon />
        <span>Move to</span>
      </DropdownMenuSubTrigger>
      <SearchableFolderSubmenuContent sessionId={sessionId} setOpen={setOpen} />
    </DropdownMenuSub>
  );
}

export function ShowInFinder({ sessionId }: { sessionId: string }) {
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
        <Icon icon="ri:finder-line" />
      )}
      <span>{isPending ? "Opening..." : "Show in Finder"}</span>
    </DropdownMenuItem>
  );
}
