import { Trans } from "@lingui/react/macro";
import { CheckCircle, PencilSimple } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import type { RefObject } from "react";
import { useCallback } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

import { useRegenerateTranscript } from "./actions";
import { TranscriptViewer } from "./renderer";
import { BatchState } from "./screens/batch";
import { TranscriptEmptyState } from "./screens/empty";
import { TranscriptListeningState } from "./screens/listening";
import { useTranscriptScreen } from "./state";

import { useListener } from "~/stt/contexts";
import { useUploadFile } from "~/stt/useUploadFile";

export function TranscriptEditButton({
  editMode,
  onEditModeChange,
}: {
  editMode: boolean;
  onEditModeChange: (editMode: boolean) => void;
}) {
  return (
    <div {...stylex.props(styles.editButtonWrapper)}>
      <button
        type="button"
        data-tauri-drag-region="false"
        aria-pressed={editMode}
        onClick={() => onEditModeChange(!editMode)}
        {...stylex.props(
          styles.editButton,
          editMode && styles.editButtonActive,
        )}
      >
        {editMode ? (
          <CheckCircle aria-hidden {...stylex.props(styles.icon)} />
        ) : (
          <PencilSimple aria-hidden {...stylex.props(styles.icon)} />
        )}
        <span {...stylex.props(styles.compactHidden)}>
          {editMode ? <Trans>Done</Trans> : <Trans>Edit</Trans>}
        </span>
      </button>
    </div>
  );
}

export function Transcript({
  sessionId,
  scrollRef,
  editMode = false,
}: {
  sessionId: string;
  scrollRef: RefObject<HTMLDivElement | null>;
  editMode?: boolean;
}) {
  return (
    <TranscriptContent
      key={sessionId}
      sessionId={sessionId}
      scrollRef={scrollRef}
      editMode={editMode}
    />
  );
}

function TranscriptContent({
  sessionId,
  scrollRef,
  editMode,
}: {
  sessionId: string;
  scrollRef: RefObject<HTMLDivElement | null>;
  editMode: boolean;
}) {
  const screen = useTranscriptScreen({ sessionId });
  const { uploadAudio, uploadTranscript } = useUploadFile(sessionId);
  const regenerateTranscript = useRegenerateTranscript(sessionId);
  const stopTranscription = useListener((state) => state.stopTranscription);
  const handleStopTranscription = useCallback(() => {
    void stopTranscription(sessionId);
  }, [sessionId, stopTranscription]);

  return (
    <div {...stylex.props(styles.root)}>
      {screen.kind === "running_batch" && (
        <TranscriptEmptyState
          isBatching
          percentage={screen.percentage}
          phase={screen.phase}
          onStopTranscription={
            screen.phase === "importing" ? undefined : handleStopTranscription
          }
        />
      )}
      {screen.kind === "batch_fallback" && (
        <BatchState
          requestedLiveTranscription={screen.requestedLiveTranscription}
          error={screen.error}
        />
      )}
      {screen.kind === "listening" && (
        <TranscriptListeningState status={screen.status} />
      )}
      {screen.kind === "empty" && (
        <TranscriptEmptyState
          isBatching={false}
          hasAudio={screen.hasAudio}
          error={screen.error}
          onRetranscribe={regenerateTranscript}
          onUploadAudio={uploadAudio}
          onUploadTranscript={uploadTranscript}
        />
      )}
      {screen.kind === "ready" && (
        <TranscriptViewer
          transcriptIds={screen.transcriptIds}
          liveSegments={screen.liveSegments}
          currentActive={screen.currentActive}
          captureGeneration={screen.captureGeneration}
          scrollRef={scrollRef}
          editMode={editMode && !screen.currentActive}
        />
      )}
    </div>
  );
}

const compact = "@container (max-width: 480px)";

const styles = stylex.create({
  compactHidden: {
    clip: {
      default: null,
      [compact]: "rect(0, 0, 0, 0)",
    },
    height: {
      default: null,
      [compact]: "1px",
    },
    margin: {
      default: null,
      [compact]: "-1px",
    },
    overflow: {
      default: null,
      [compact]: "hidden",
    },
    padding: {
      default: null,
      [compact]: 0,
    },
    position: {
      default: null,
      [compact]: "absolute",
    },
    whiteSpace: {
      default: null,
      [compact]: "nowrap",
    },
    width: {
      default: null,
      [compact]: "1px",
    },
  },
  editButton: {
    alignItems: "center",
    backgroundColor: {
      default: colors.card,
      ":hover": colors.accent,
    },
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 2px ${colors.ring}`,
    },
    color: colors.foreground,
    display: "flex",
    fontSize: "0.875rem",
    fontWeight: 500,
    gap: {
      default: "0.375rem",
      [compact]: 0,
    },
    height: "1.75rem",
    justifyContent: {
      default: null,
      [compact]: "center",
    },
    outline: {
      default: null,
      ":focus-visible": "none",
    },
    paddingInline: {
      default: "0.5rem",
      [compact]: 0,
    },
    transitionDuration: "150ms",
    transitionProperty: "color, background-color, border-color",
    width: {
      default: null,
      [compact]: "1.75rem",
    },
  },
  editButtonActive: {
    backgroundColor: `color-mix(in oklab, ${colors.primary} 10%, transparent)`,
    borderColor: `color-mix(in oklab, ${colors.primary} 30%, transparent)`,
    color: colors.primary,
  },
  editButtonWrapper: {
    flexShrink: 0,
    marginRight: "0.25rem",
  },
  icon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    overflow: "hidden",
    position: "relative",
  },
});
