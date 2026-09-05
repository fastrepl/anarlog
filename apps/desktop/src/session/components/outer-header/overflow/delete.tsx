import { Trans } from "@lingui/react/macro";
import { useCallback } from "react";

import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import { Trash } from "@anlg/ui/components/icons";
import { DropdownMenuItem } from "@anlg/ui/components/ui/dropdown-menu";
import { cn } from "@anlg/utils";

import { useDeleteSession } from "~/session/hooks/useDeleteSession";
import { useSessionSummary } from "~/session/queries";

export function DeleteNote({ sessionId }: { sessionId: string }) {
  const deleteSession = useDeleteSession();
  const title = useSessionSummary(sessionId)?.title;

  const handleDeleteNote = useCallback(() => {
    deleteSession(sessionId, { title });

    void analyticsCommands.event({
      event: "session_deleted",
      includes_recording: true,
    });
  }, [sessionId, deleteSession, title]);

  return (
    <DropdownMenuItem
      onClick={handleDeleteNote}
      className={cn([
        "cursor-pointer text-red-600 dark:text-red-400",
        "hover:bg-red-50 hover:text-red-700 dark:hover:bg-red-950/50 dark:hover:text-red-300",
      ])}
    >
      <Trash />
      <span>
        <Trans>Delete</Trans>
      </span>
    </DropdownMenuItem>
  );
}
