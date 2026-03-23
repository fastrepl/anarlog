import { Loader2Icon, TrashIcon } from "lucide-react";
import { useCallback } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { DropdownMenuItem } from "@hypr/ui/components/ui/dropdown-menu";
import { cn } from "@hypr/utils";

import { useAudioPlayer } from "~/audio-player";
import {
  captureSessionData,
  deleteSessionCascade,
} from "~/store/tinybase/store/deleteSession";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";
import { useUndoDelete } from "~/store/zustand/undo-delete";
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
        "cursor-pointer text-red-600",
        "hover:bg-red-50 hover:text-red-700",
      ])}
    >
      {isDeletingRecording ? (
        <Loader2Icon className="animate-spin" />
      ) : (
        <TrashIcon />
      )}
      <span>{isDeletingRecording ? "Deleting..." : "Delete recording"}</span>
    </DropdownMenuItem>
  );
}

export function DeleteNote({ sessionId }: { sessionId: string }) {
  const store = main.UI.useStore(main.STORE_ID);
  const indexes = main.UI.useIndexes(main.STORE_ID);
  const invalidateResource = useTabs((state) => state.invalidateResource);
  const addDeletion = useUndoDelete((state) => state.addDeletion);

  const handleDeleteNote = useCallback(() => {
    if (!store) {
      return;
    }

    const capturedData = captureSessionData(store, indexes, sessionId);

    invalidateResource("sessions", sessionId);
    void deleteSessionCascade(store, indexes, sessionId, {
      skipAudio: true,
    });

    if (capturedData) {
      addDeletion(capturedData, () => {
        void fsSyncCommands.audioDelete(sessionId);
      });
    }

    void analyticsCommands.event({
      event: "session_deleted",
      includes_recording: true,
    });
  }, [store, indexes, sessionId, invalidateResource, addDeletion]);

  return (
    <DropdownMenuItem
      onClick={handleDeleteNote}
      className={cn([
        "cursor-pointer text-red-600",
        "hover:bg-red-50 hover:text-red-700",
      ])}
    >
      <TrashIcon />
      <span>Delete</span>
    </DropdownMenuItem>
  );
}
