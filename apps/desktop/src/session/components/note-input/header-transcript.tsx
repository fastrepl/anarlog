import { useLingui } from "@lingui/react/macro";
import { Waveform } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useMemo } from "react";

import { DancingSticks } from "@anlg/ui/components/ui/dancing-sticks";
import { Spinner } from "@anlg/ui/components/ui/spinner";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { IconHeaderView, copyTextToClipboard } from "./header-shared";

import * as AudioPlayer from "~/audio-player";
import { useRegenerateTranscript } from "~/session/components/note-input/transcript/actions";
import {
  buildTranscriptExportSegments,
  formatTranscriptExportSegments,
} from "~/session/components/note-input/transcript/export-data";
import { useSessionTranscriptRenderData } from "~/session/components/note-input/transcript/render-request-hooks";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import { useListener } from "~/stt/contexts";
import { useStartListening } from "~/stt/useStartListening";
import {
  isMainWebviewWindow,
  requestMainListenerControl,
} from "~/stt/window-control";

export function HeaderViewTranscript({
  isActive,
  isTranscribing,
  onClick = () => {},
  sessionId,
}: {
  isActive: boolean;
  isTranscribing: boolean;
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
}: {
  isActive: boolean;
  isTranscribing: boolean;
  onClick?: () => void;
  onContextMenu?: React.MouseEventHandler<HTMLButtonElement>;
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
          <Spinner size={16} sx={styles.shrinkIcon} />
        ) : (
          <Waveform {...stylex.props(styles.icon)} />
        )
      }
      onClick={onClick}
      onContextMenu={onContextMenu}
      title={undefined}
      sx={
        live && isActive
          ? [styles.liveActive, live.degraded ? styles.degraded : styles.live]
          : undefined
      }
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
    <span {...stylex.props(styles.liveIcon)}>
      {live.muted ? (
        <Waveform {...stylex.props(styles.icon)} />
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

const compact = "@container (max-width: 480px)";

const styles = stylex.create({
  degraded: {
    backgroundColor: {
      default: "rgb(255 251 235)",
      ":hover": "rgb(254 243 199)",
      ":is(.dark *)": "rgb(69 26 3 / 0.5)",
      ":is(.dark *):hover": "rgb(69 26 3)",
    },
    color: {
      default: "rgb(245 158 11)",
      ":hover": "rgb(217 119 6)",
      ":is(.dark *)": "rgb(252 211 77)",
      ":is(.dark *):hover": "rgb(253 230 138)",
    },
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  live: {
    backgroundColor: {
      default: "rgb(254 242 242)",
      ":hover": "rgb(254 226 226)",
      ":is(.dark *)": "rgb(69 10 10 / 0.5)",
      ":is(.dark *):hover": "rgb(69 10 10)",
    },
    color: {
      default: "rgb(239 68 68)",
      ":hover": "rgb(220 38 38)",
      ":is(.dark *)": "rgb(252 165 165)",
      ":is(.dark *):hover": "rgb(254 202 202)",
    },
  },
  liveActive: {
    gap: {
      default: "0.375rem",
      [compact]: 0,
    },
    minWidth: {
      default: "98px",
      [compact]: "2.5rem",
    },
    paddingInline: "0.5rem",
    width: {
      default: "98px",
      [compact]: "2.5rem",
    },
  },
  liveIcon: {
    alignItems: "center",
    display: "flex",
    height: "1rem",
    justifyContent: "center",
    position: "relative",
    width: "1rem",
  },
  shrinkIcon: {
    flexShrink: 0,
  },
});

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
  onClick,
  sessionId,
  live,
}: {
  isActive: boolean;
  isTranscribing: boolean;
  onClick?: () => void;
  sessionId: string;
  live?: {
    amplitude: number;
    degraded: boolean;
    muted: boolean;
  };
}) {
  const regenerate = useRegenerateTranscript(sessionId);
  const startListening = useStartListening(sessionId);
  const { request: transcriptExportRequest } =
    useSessionTranscriptRenderData(sessionId);
  const {
    audioExists,
    audioExistsResolved,
    deleteRecording,
    isDeletingRecording,
  } = AudioPlayer.useAudioPlayer();
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
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

    if (sessionMode === "inactive") {
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
      onClick={onClick}
      onContextMenu={showContextMenu}
      live={live}
    />
  );
}
