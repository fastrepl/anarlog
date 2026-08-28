import { Trans } from "@lingui/react/macro";
import { CircleNotch, Trash } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback } from "react";

import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import { DropdownMenuItem } from "@anlg/ui/components/ui/dropdown-menu";

import { useAudioPlayer } from "~/audio-player";
import { useDeleteSession } from "~/session/hooks/useDeleteSession";
import { useSessionSummary } from "~/session/queries";
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
      sx={styles.destructiveItem}
    >
      {isDeletingRecording ? (
        <CircleNotch {...stylex.props(styles.spinner)} />
      ) : (
        <Trash />
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
  const title = useSessionSummary(sessionId)?.title;

  const handleDeleteNote = useCallback(() => {
    deleteSession(sessionId, { title });

    void analyticsCommands.event({
      event: "session_deleted",
      includes_recording: true,
    });
  }, [sessionId, deleteSession, title]);

  return (
    <DropdownMenuItem onClick={handleDeleteNote} sx={styles.destructiveItem}>
      <Trash />
      <span>
        <Trans>Delete</Trans>
      </span>
    </DropdownMenuItem>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  destructiveItem: {
    backgroundColor: {
      default: "transparent",
      ":hover": "#fef2f2",
    },
    color: {
      default: "#dc2626",
      ":hover": "#b91c1c",
    },
    cursor: "pointer",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
});
