import { useLingui } from "@lingui/react/macro";
import { useCallback, useMemo } from "react";

import { CheckCircle, PencilSimple } from "@anlg/ui/components/icons";
import { DancingSticks } from "@anlg/ui/components/ui/dancing-sticks";
import { Spinner } from "@anlg/ui/components/ui/spinner";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn, safeParseDate } from "@anlg/utils";

import { IconHeaderView, copyTextToClipboard } from "./header-shared";
import { TranscriptAudioIcon } from "./header-transcript-icon";

import * as AudioPlayer from "~/audio-player";
import { useNow } from "~/calendar/hooks";
import { useRegenerateTranscript } from "~/session/components/note-input/transcript/actions";
import {
  buildTranscriptExportSegments,
  formatTranscriptExportSegments,
} from "~/session/components/note-input/transcript/export-data";
import { useSessionTranscriptRenderData } from "~/session/components/note-input/transcript/render-request-hooks";
import { useHasTranscript } from "~/session/components/shared";
import { useSessionEvent } from "~/session/hooks/useSessionEvent";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import { useListener } from "~/stt/contexts";
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
    degraded: boolean;
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
                ? live.degraded
                  ? [
                      "bg-amber-50 text-amber-500 hover:bg-amber-100 hover:text-amber-600",
                      "dark:bg-amber-950/50 dark:text-amber-300 dark:hover:bg-amber-950 dark:hover:text-amber-200",
                    ]
                  : [
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
    degraded: boolean;
    muted: boolean;
  };
}) {
  const color = live.degraded ? "#f59e0b" : "#ef4444";

  return (
    <span className="relative flex size-4 items-center justify-center">
      {live.muted ? (
        <TranscriptAudioIcon />
      ) : (
        <DancingSticks
          amplitude={live.amplitude}
          color={color}
          height={16}
          width={16}
        />
      )}
    </span>
  );
}

function useTranscriptLiveViewState(sessionId: string) {
  const { amplitude, degraded, mode, muted } = useListener((state) => {
    const mode = state.getSessionMode(sessionId);
    return {
      amplitude: state.live.amplitude,
      degraded: state.live.degraded,
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
            degraded: Boolean(degraded),
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
    degraded: boolean;
    muted: boolean;
  };
}) {
  const regenerate = useRegenerateTranscript(sessionId);
  const startListening = useStartListeningWithBatchOverride(sessionId);
  const sessionEvent = useSessionEvent(sessionId);
  const hasTranscript = useHasTranscript(sessionId);
  const now = useNow();
  const { request: transcriptExportRequest } =
    useSessionTranscriptRenderData(sessionId);
  const {
    audioExists,
    audioExistsResolved,
    deleteRecording,
    isDeletingRecording,
  } = AudioPlayer.useAudioPlayer();
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const endedAt = sessionEvent?.ended_at
    ? safeParseDate(sessionEvent.ended_at)
    : null;
  const eventEnded = !!endedAt && endedAt.getTime() <= now.getTime();
  const canEdit =
    sessionMode === "inactive" &&
    hasTranscript &&
    (!sessionEvent || eventEnded) &&
    Boolean(onEditModeChange);
  const handleClick = useCallback(() => {
    if (canEdit) {
      onEditModeChange?.(!editMode);
      return;
    }

    onClick?.();
  }, [canEdit, editMode, onClick, onEditModeChange]);
  const canCopyTranscript = Boolean(transcriptExportRequest);
  const handleCopyTranscript = useCallback(async () => {
    if (!transcriptExportRequest) {
      return;
    }

    try {
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
      sonnerToast.error("Failed to copy transcript");
    }
  }, [transcriptExportRequest]);
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
