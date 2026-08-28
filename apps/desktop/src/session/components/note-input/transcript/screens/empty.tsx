import { t } from "@lingui/core/macro";
import {
  ArrowsClockwise,
  Square,
  WarningCircle,
  Waveform,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { colors } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import { Spinner } from "@anlg/ui/components/ui/spinner";

export function TranscriptEmptyState({
  isBatching,
  hasAudio,
  percentage,
  phase,
  error,
  onRetranscribe,
  onUploadAudio,
  onUploadTranscript,
  onStopTranscription,
}: {
  isBatching?: boolean;
  hasAudio?: boolean;
  percentage?: number;
  phase?: "importing" | "transcribing";
  error?: string | null;
  onRetranscribe?: () => void;
  onUploadAudio?: () => void;
  onUploadTranscript?: () => void;
  onStopTranscription?: () => void;
}) {
  if (error) {
    return (
      <div role="alert" {...stylex.props(styles.root)}>
        <WarningCircle aria-hidden {...stylex.props(styles.stateIcon)} />
        <div {...stylex.props(styles.copy, styles.copyWithActions)}>
          <p {...stylex.props(styles.title)}>{t`Transcription failed`}</p>
          <p {...stylex.props(styles.description)}>{error}</p>
        </div>
        {onRetranscribe && (
          <Button size="sm" sx={styles.button} onClick={onRetranscribe}>
            <ArrowsClockwise {...stylex.props(styles.buttonIcon)} />
            {t`Re-transcribe`}
          </Button>
        )}
      </div>
    );
  }

  if (isBatching) {
    const hasProgress = typeof percentage === "number" && percentage > 0;

    return (
      <div role="status" {...stylex.props(styles.root)}>
        <div {...stylex.props(styles.spinner)}>
          <Spinner size={36} />
        </div>
        <div {...stylex.props(onStopTranscription && styles.copyWithActions)}>
          <p {...stylex.props(styles.title)}>
            {phase === "importing"
              ? t`Importing audio...`
              : t`Generating transcript...`}
          </p>
          {hasProgress && (
            <p {...stylex.props(styles.progress)}>
              {t`${Math.round((percentage ?? 0) * 100)}% complete`}
            </p>
          )}
        </div>
        {onStopTranscription && (
          <Button
            variant="outline"
            size="sm"
            sx={styles.button}
            onClick={onStopTranscription}
          >
            <Square {...stylex.props(styles.stopIcon)} weight="fill" />
            {t`Stop transcription`}
          </Button>
        )}
      </div>
    );
  }

  return (
    <div {...stylex.props(styles.root)}>
      <Waveform aria-hidden {...stylex.props(styles.stateIcon)} />
      <div {...stylex.props(styles.copy, styles.copyWithActions)}>
        <p {...stylex.props(styles.title)}>
          {hasAudio ? t`Audio available` : t`No transcript available`}
        </p>
        <p {...stylex.props(styles.description)}>
          {hasAudio
            ? t`Re-transcribe this audio, or upload a transcript file.`
            : t`Upload audio or a transcript file to populate this note.`}
        </p>
      </div>
      {(onRetranscribe || onUploadAudio || onUploadTranscript) && (
        <div {...stylex.props(styles.actions)}>
          {hasAudio && onRetranscribe && (
            <Button size="sm" sx={styles.button} onClick={onRetranscribe}>
              <ArrowsClockwise {...stylex.props(styles.buttonIcon)} />
              {t`Re-transcribe`}
            </Button>
          )}
          {!hasAudio && onUploadAudio && (
            <Button variant="outline" size="sm" onClick={onUploadAudio}>
              {t`Upload audio`}
            </Button>
          )}
          {onUploadTranscript && (
            <Button variant="outline" size="sm" onClick={onUploadTranscript}>
              {t`Upload transcript`}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}

const styles = stylex.create({
  actions: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  button: {
    gap: "0.5rem",
  },
  buttonIcon: {
    height: "1rem",
    width: "1rem",
  },
  copy: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
    maxWidth: "28rem",
  },
  copyWithActions: {
    marginBottom: "1.5rem",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: 1.625,
  },
  progress: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    fontVariantNumeric: "tabular-nums",
    lineHeight: 1.625,
    marginTop: "0.5rem",
  },
  root: {
    alignItems: "center",
    display: "flex",
    flexDirection: "column",
    height: "100%",
    justifyContent: "center",
    minHeight: "400px",
    paddingInline: "1.5rem",
    textAlign: "center",
  },
  spinner: {
    color: colors.mutedForeground,
    marginBottom: "1.25rem",
  },
  stateIcon: {
    color: colors.mutedForeground,
    height: "2.25rem",
    marginBottom: "1.25rem",
    strokeWidth: 1.5,
    width: "2.25rem",
  },
  stopIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  title: {
    fontSize: "1rem",
    fontWeight: 500,
  },
});

export { styles as transcriptEmptyStateStyles };
