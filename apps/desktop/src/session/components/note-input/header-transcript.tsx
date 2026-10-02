import { useLingui } from "@lingui/react/macro";
import { useCallback, useMemo } from "react";

import { CheckCircle, PencilSimple } from "@anlg/ui/components/icons";
import { DancingSticks } from "@anlg/ui/components/ui/dancing-sticks";
import { Spinner } from "@anlg/ui/components/ui/spinner";
import { toast } from "@anlg/ui/components/ui/toast";
import { cn } from "@anlg/utils";

import { IconHeaderView, copyTextToClipboard } from "./header-shared";
import { TranscriptAudioIcon } from "./header-transcript-icon";

import * as AudioPlayer from "~/audio-player";
import { useRegenerateTranscript } from "~/session/components/note-input/transcript/actions";
import {
  buildTranscriptExportSegments,
  formatTranscriptExportSegments,
} from "~/session/components/note-input/transcript/export-data";
import { getSessionTranscriptRenderRequest } from "~/session/components/note-input/transcript/render-request-hooks";
import { useHasTranscript } from "~/session/components/shared";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import { useListener } from "~/stt/contexts";
import { useSessionTranscriptMetadata } from "~/stt/queries";
import { useStartListeningWithBatchOverride } from "~/stt/useStartListeningWithBatchOverride";
import {
  isMainWebviewWindow,
  requestMainListenerControl,
} from "~/stt/window-control";

export function HeaderViewTranscript({
  isActive,
  isTranscribing,
  editMode = false,
  onEditModeChange,
  onClick = () => {},
  sessionId,
}: {
  isActive: boolean;
  isTranscribing: boolean;
  editMode?: boolean;
  onEditModeChange?: (editMode: boolean) => void;
  onClick?: () => void;
  sessionId: string;
}) {
  const liveState = useTranscriptLiveViewState(sessionId);

  if (!isActive) {
    return (
      <HeaderViewTranscriptButton
        isActive={isActive}
        isTranscribing={isTranscribing}
        onClick={onClick}
        live={liveState.live}
      />
    );
  }

  return (
    <HeaderViewTranscriptActive
      isActive={isActive}
      isTranscribing={isTranscribing}
      editMode={editMode}
      onEditModeChange={onEditModeChange}
      onClick={onClick}
      sessionId={sessionId}
      live={liveState.live}
    />
  );
}

function HeaderViewTranscriptButton({
  isActive,
  isTranscribing,
  onClick,
  onContextMenu,
  live,
  suffixIcon,
  pressed,
}: {
  isActive: boolean;
  isTranscribing: boolean;
  onClick?: () => void;
  onContextMenu?: React.MouseEventHandler<HTMLButtonElement>;
  suffixIcon?: React.ReactNode;
  pressed?: boolean;
  live?: {
    amplitude: number;
    muted: boolean;
  };
}) {
  const { t } = useLingui();

  return (
    <IconHeaderView
      isActive={isActive}
      label={t`Transcript`}
      hoverLabel={undefined}
      icon={
        live ? (
          <HeaderViewTranscriptLiveIcon live={live} />
        ) : isTranscribing ? (
          <Spinner size={16} className="shrink-0" />
        ) : (
          <TranscriptAudioIcon />
        )
      }
      suffixIcon={suffixIcon}
      pressed={pressed}
      onClick={onClick}
      onContextMenu={onContextMenu}
      title={undefined}
      className={cn([
        live
          ? [
              "group/transcript-live",
              isActive
                ? "w-[98px] min-w-[98px] gap-1.5 px-2 @max-[480px]:w-10 @max-[480px]:min-w-10 @max-[480px]:gap-0"
                : null,
              isActive
                ? [
                    "bg-red-50 text-red-500 hover:bg-red-100 hover:text-red-600",
                    "dark:bg-red-950/50 dark:text-red-300 dark:hover:bg-red-950 dark:hover:text-red-200",
                  ]
                : null,
            ]
          : null,
      ])}
    />
  );
}

function HeaderViewTranscriptLiveIcon({
  live,
}: {
  live: {
    amplitude: number;
    muted: boolean;
  };
}) {
  return (
    <span className="relative flex size-4 items-center justify-center">
      {live.muted ? (
        <TranscriptAudioIcon />
      ) : (
        <DancingSticks
          amplitude={live.amplitude}
          color="#ef4444"
          height={16}
          width={16}
        />
      )}
    </span>
  );
}

function useTranscriptLiveViewState(sessionId: string) {
  const { amplitude, mode, muted } = useListener((state) => {
    const mode = state.getSessionMode(sessionId);
    return {
      amplitude: state.live.amplitude,
      mode,
      muted: state.live.muted,
    };
  });
  return {
    live:
      mode === "active"
        ? {
            amplitude: Math.min(
              Math.hypot(amplitude.mic, amplitude.speaker),
              1,
            ),
            muted,
          }
        : undefined,
  };
}

function HeaderViewTranscriptActive({
  isActive,
  isTranscribing,
  editMode,
  onEditModeChange,
  onClick,
  sessionId,
  live,
}: {
  isActive: boolean;
  isTranscribing: boolean;
  editMode: boolean;
  onEditModeChange?: (editMode: boolean) => void;
  onClick?: () => void;
  sessionId: string;
  live?: {
    amplitude: number;
    muted: boolean;
  };
}) {
  const regenerate = useRegenerateTranscript(sessionId);
  const startListening = useStartListeningWithBatchOverride(sessionId);
  const hasTranscript = useHasTranscript(sessionId);
  const transcriptMetadata = useSessionTranscriptMetadata(sessionId);
  const {
    audioExists,
    audioExistsResolved,
    deleteRecording,
    isDeletingRecording,
  } = AudioPlayer.useAudioPlayer();
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const canEdit =
    sessionMode === "inactive" && hasTranscript && Boolean(onEditModeChange);
  const handleClick = useCallback(() => {
    if (canEdit) {
      onEditModeChange?.(!editMode);
      return;
    }

    onClick?.();
  }, [canEdit, editMode, onClick, onEditModeChange]);
  const canCopyTranscript = transcriptMetadata.some((t) => t.hasWords);
  const handleCopyTranscript = useCallback(async () => {
    try {
      const transcriptExportRequest =
        await getSessionTranscriptRenderRequest(sessionId);
      if (!transcriptExportRequest) {
        return;
      }

      const transcriptSegments = await buildTranscriptExportSegments(
        transcriptExportRequest,
      );
      const transcriptText = formatTranscriptExportSegments(transcriptSegments);
      if (!transcriptText) {
        return;
      }

      await copyTextToClipboard(transcriptText, {
        success: "Transcript copied to clipboard",
        error: "Failed to copy transcript",
      });
    } catch (error) {
      console.error("Failed to copy transcript", error);
      toast.error("Failed to copy transcript");
    }
  }, [sessionId]);
  const handleDeleteRecording = useCallback(() => {
    void deleteRecording();
  }, [deleteRecording]);
  const handleResumeListening = useCallback(() => {
    if (!isMainWebviewWindow()) {
      void requestMainListenerControl("start", sessionId);
      return;
    }

    void startListening();
  }, [sessionId, startListening]);
  const contextMenu = useMemo<MenuItemDef[]>(() => {
    const items: MenuItemDef[] = [
      {
        id: `copy-transcript-${sessionId}`,
        text: "Copy",
        action: () => {
          void handleCopyTranscript();
        },
        disabled: !canCopyTranscript,
      },
    ];

    if (sessionMode === "inactive" || sessionMode === "running_batch") {
      items.push({
        id: `resume-listening-${sessionId}`,
        text: "Resume listening",
        action: handleResumeListening,
      });
    }

    if (audioExistsResolved && sessionMode === "inactive" && audioExists) {
      items.push({
        id: `regenerate-transcript-${sessionId}`,
        text: "Re-transcribe",
        action: () => {
          void regenerate();
        },
      });
    }

    if (audioExists) {
      items.push({
        id: `delete-recording-${sessionId}`,
        text: "Delete recording",
        action: handleDeleteRecording,
        disabled: isDeletingRecording,
      });
    }

    return items;
  }, [
    audioExists,
    audioExistsResolved,
    canCopyTranscript,
    handleCopyTranscript,
    handleDeleteRecording,
    handleResumeListening,
    isDeletingRecording,
    regenerate,
    sessionMode,
    sessionId,
  ]);
  const showContextMenu = useNativeContextMenu(contextMenu);

  return (
    <HeaderViewTranscriptButton
      isActive={isActive}
      isTranscribing={isTranscribing}
      onClick={handleClick}
      onContextMenu={showContextMenu}
      live={live}
      suffixIcon={
        canEdit ? (
          editMode ? (
            <CheckCircle aria-hidden className="size-3.5" />
          ) : (
            <PencilSimple aria-hidden className="size-3.5" />
          )
        ) : undefined
      }
      pressed={canEdit ? editMode : undefined}
    />
  );
}
