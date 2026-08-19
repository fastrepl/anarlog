import { t } from "@lingui/core/macro";
import {
  ArrowsClockwise,
  Square,
  WarningCircle,
  Waveform,
} from "@phosphor-icons/react";

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
      <div
        role="alert"
        className="flex h-full min-h-[400px] flex-col items-center justify-center px-6 text-center"
      >
        <WarningCircle
          aria-hidden
          className="text-muted-foreground mb-5 size-9 stroke-[1.5]"
        />
        <div className="mb-6 flex max-w-md flex-col gap-2">
          <p className="text-base font-medium">{t`Transcription failed`}</p>
          <p className="text-muted-foreground text-sm leading-relaxed">
            {error}
          </p>
        </div>
        {onRetranscribe && (
          <Button size="sm" className="gap-2" onClick={onRetranscribe}>
            <ArrowsClockwise className="size-4" />
            {t`Re-transcribe`}
          </Button>
        )}
      </div>
    );
  }

  if (isBatching) {
    const hasProgress = typeof percentage === "number" && percentage > 0;

    return (
      <div
        role="status"
        className="flex h-full min-h-[400px] flex-col items-center justify-center px-6 text-center"
      >
        <div className="text-muted-foreground mb-5">
          <Spinner size={36} />
        </div>
        <div className={onStopTranscription ? "mb-6" : undefined}>
          <p className="text-base font-medium">
            {phase === "importing"
              ? t`Importing audio...`
              : t`Generating transcript...`}
          </p>
          {hasProgress && (
            <p className="text-muted-foreground mt-2 text-sm leading-relaxed tabular-nums">
              {t`${Math.round((percentage ?? 0) * 100)}% complete`}
            </p>
          )}
        </div>
        {onStopTranscription && (
          <Button
            variant="outline"
            size="sm"
            className="gap-2"
            onClick={onStopTranscription}
          >
            <Square className="size-3" weight="fill" />
            {t`Stop transcription`}
          </Button>
        )}
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-[400px] flex-col items-center justify-center px-6 text-center">
      <Waveform
        aria-hidden
        className="text-muted-foreground mb-5 size-9 stroke-[1.5]"
      />
      <div className="mb-6 flex max-w-md flex-col gap-2">
        <p className="text-base font-medium">
          {hasAudio ? t`Audio available` : t`No transcript available`}
        </p>
        <p className="text-muted-foreground text-sm leading-relaxed">
          {hasAudio
            ? t`Re-transcribe this audio, or upload a transcript file.`
            : t`Upload audio or a transcript file to populate this note.`}
        </p>
      </div>
      {(onRetranscribe || onUploadAudio || onUploadTranscript) && (
        <div className="flex items-center gap-2">
          {hasAudio && onRetranscribe && (
            <Button size="sm" className="gap-2" onClick={onRetranscribe}>
              <ArrowsClockwise className="size-4" />
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
