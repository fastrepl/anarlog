import { Trans } from "@lingui/react/macro";
import { Loader2Icon, TrashIcon } from "lucide-react";
import { useCallback } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { DropdownMenuItem } from "@hypr/ui/components/ui/dropdown-menu";
import { cn } from "@hypr/utils";

import { useAudioPlayer } from "~/audio-player";
import { useDeleteSession } from "~/session/hooks/useDeleteSession";
import { useListener } from "~/stt/contexts";

export function DeleteRecording({ sessionId }: { sessionId: string }) {
  const { deleteRecording, isDeletingRecording } = useAudioPlayer();
  const mode = useListener((state) => state.getSessionMode(sessionId));
  const isDisabled =
    isDeletingRecording ||
    mode === "active" ||
    mode === "finalizing" ||
    mode === "running_batch";

  const handleDeleteRecording = useCallback(() => {
    void deleteRecording();
  }, [deleteRecording]);

  return (
    <DropdownMenuItem
      onClick={handleDeleteRecording}
      disabled={isDisabled}
      className={cn([
        "cursor-pointer text-red-600 dark:text-red-400",
        "hover:bg-red-50 hover:text-red-700 dark:hover:bg-red-950/50 dark:hover:text-red-300",
      ])}
    >
      {isDeletingRecording ? (
        <Loader2Icon className="animate-spin" />
      ) : (
        <TrashIcon />
      )}
      <span>
        {isDeletingRecording ? (
          <Trans>Deleting...</Trans>
        ) : (
          <Trans>Delete recording</Trans>
        )}
      </span>
    </DropdownMenuItem>
  );
}

export function DeleteNote({ sessionId }: { sessionId: string }) {
  const deleteSession = useDeleteSession();

  const handleDeleteNote = useCallback(() => {
    deleteSession(sessionId);

    void analyticsCommands.event({
      event: "session_deleted",
      includes_recording: true,
    });
  }, [sessionId, deleteSession]);

  return (
    <DropdownMenuItem
      onClick={handleDeleteNote}
      className={cn([
        "cursor-pointer text-red-600 dark:text-red-400",
        "hover:bg-red-50 hover:text-red-700 dark:hover:bg-red-950/50 dark:hover:text-red-300",
      ])}
    >
      <TrashIcon />
      <span>
        <Trans>Delete</Trans>
      </span>
    </DropdownMenuItem>
  );
}
