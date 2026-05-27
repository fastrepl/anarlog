import {
  Loader2Icon,
  Pencil,
  RefreshCw,
  SquareIcon,
  TrashIcon,
} from "lucide-react";
import { type ReactNode, useCallback, useRef } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { Button } from "@hypr/ui/components/ui/button";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import * as AudioPlayer from "~/audio-player";
import { getEnhancerService } from "~/services/enhancer";
import { Transcript } from "~/session/components/note-input/transcript";
import { useTranscriptScreen } from "~/session/components/note-input/transcript/state";
import { useListener } from "~/stt/contexts";
import { isStoppedTranscriptionError, useRunBatch } from "~/stt/useRunBatch";
import { useUploadFile } from "~/stt/useUploadFile";

export function PostSessionAccessory({
  sessionId,
  hasAudio,
  hasTranscript,
  isTranscriptExpanded,
  fillHeight = false,
}: {
  sessionId: string;
  hasAudio: boolean;
  hasTranscript: boolean;
  isTranscriptExpanded: boolean;
  fillHeight?: boolean;
}) {
  const screen = useTranscriptScreen({ sessionId });
  const isBatching = screen.kind === "running_batch";
  const timeline = isBatching ? (
    <BatchProgressTimeline sessionId={sessionId} screen={screen} />
  ) : hasAudio ? (
    <AudioPlayer.Timeline />
  ) : null;

  if (!isTranscriptExpanded && !timeline) {
    return null;
  }

  return (
    <div
      className={cn([
        "flex min-h-0 flex-col",
        fillHeight && "h-full overflow-hidden",
      ])}
    >
      {isTranscriptExpanded ? (
        <div
          className={cn([
            fillHeight ? "min-h-0 flex-1 overflow-hidden" : "shrink-0",
          ])}
        >
          <TranscriptPanel
            sessionId={sessionId}
            screen={screen}
            hasAudio={hasAudio}
            hasTranscript={hasTranscript}
            isExpanded={isTranscriptExpanded}
            fillHeight={fillHeight}
          />
        </div>
      ) : null}
      {timeline ? <TimelineSlot>{timeline}</TimelineSlot> : null}
    </div>
  );
}

function TimelineSlot({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-14 w-full shrink-0 items-center">{children}</div>
  );
}

function TranscriptPanel({
  sessionId,
  screen,
  hasAudio,
  hasTranscript,
  isExpanded,
  fillHeight,
}: {
  sessionId: string;
  screen: ReturnType<typeof useTranscriptScreen>;
  hasAudio: boolean;
  hasTranscript: boolean;
  isExpanded: boolean;
  fillHeight: boolean;
}) {
  if (screen.kind === "running_batch") {
    return (
      <BatchingTranscriptPanel
        sessionId={sessionId}
        screen={screen}
        isExpanded={isExpanded}
        fillHeight={fillHeight}
      />
    );
  }

  if (hasTranscript) {
    return (
      <TranscriptReadyPanel
        sessionId={sessionId}
        isExpanded={isExpanded}
        fillHeight={fillHeight}
      />
    );
  }

  return (
    <TranscriptEmptyPanel
      sessionId={sessionId}
      hasAudio={hasAudio}
      isExpanded={isExpanded}
      fillHeight={fillHeight}
    />
  );
}

function useRegenerateTranscript(sessionId: string) {
  const runBatch = useRunBatch(sessionId);
  const handleBatchFailed = useListener((state) => state.handleBatchFailed);

  return useCallback(async () => {
    const result = await fsSyncCommands.audioPath(sessionId);
    if (result.status === "error") return;

    const audioPath = result.data;

    try {
      await runBatch(audioPath);
      getEnhancerService()?.queueAutoEnhanceIfSummaryEmpty(sessionId);
    } catch (error) {
      if (isStoppedTranscriptionError(error)) {
        return;
      }
      const msg = error instanceof Error ? error.message : String(error);
      handleBatchFailed(sessionId, msg);
    }
  }, [handleBatchFailed, runBatch, sessionId]);
}

function BatchingTranscriptPanel({
  sessionId,
  screen,
  isExpanded,
  fillHeight,
}: {
  sessionId: string;
  screen: {
    kind: "running_batch";
    percentage?: number;
    phase?: "importing" | "transcribing";
  };
  isExpanded: boolean;
  fillHeight: boolean;
}) {
  const stopTranscription = useListener((state) => state.stopTranscription);
  const handleStop = useCallback(() => {
    void stopTranscription(sessionId);
  }, [sessionId, stopTranscription]);
  const { percentage, phase } = screen;
  const phaseLabel = phase === "importing" ? "Importing..." : "Transcribing...";
  const canStopTranscription = phase !== "importing";

  if (!isExpanded) {
    return null;
  }

  return (
    <TranscriptCard fillHeight={fillHeight}>
      <div className="flex shrink-0 items-center justify-between px-3 py-1.5">
        <span className="text-xs font-medium text-neutral-500">Transcript</span>
        <div className="flex items-center gap-1 px-1 py-0.5">
          <Spinner size={10} />
          <span className="text-[11px] text-neutral-500">
            {phaseLabel}
            {typeof percentage === "number" && percentage > 0 && (
              <span className="ml-1 text-neutral-400 tabular-nums">
                {Math.round(percentage * 100)}%
              </span>
            )}
          </span>
          {canStopTranscription ? (
            <StopTranscriptionButton onClick={handleStop} compact />
          ) : null}
        </div>
      </div>

      <BatchTranscriptSkeleton fillHeight={fillHeight} />
    </TranscriptCard>
  );
}

function BatchTranscriptSkeleton({ fillHeight }: { fillHeight: boolean }) {
  const rows = [
    {
      speaker: "w-16",
      time: "w-8",
      lines: ["w-[74%]", "w-[54%]"],
    },
    {
      speaker: "w-12",
      time: "w-10",
      lines: ["w-[62%]", "w-[82%]", "w-[38%]"],
    },
    {
      speaker: "w-20",
      time: "w-8",
      lines: ["w-[70%]", "w-[48%]"],
    },
  ] as const;

  return (
    <div
      aria-hidden
      data-testid="transcript-skeleton"
      className={cn([
        "flex flex-col overflow-hidden px-6 py-4",
        fillHeight
          ? "min-h-0 flex-1 justify-center"
          : "h-[178px] justify-start",
      ])}
    >
      <div className="flex w-full max-w-[940px] flex-col gap-8">
        {rows.map((row, index) => (
          <div key={index} className="flex gap-4">
            <div className="flex w-[72px] shrink-0 flex-col gap-3 pt-0.5">
              <div
                className={cn([
                  "h-2.5 rounded-full bg-neutral-200/80",
                  "animate-pulse",
                  row.speaker,
                ])}
              />
              <div
                className={cn([
                  "h-1.5 rounded-full bg-neutral-100",
                  "animate-pulse",
                  row.time,
                ])}
              />
            </div>
            <div className="flex min-w-0 flex-1 flex-col gap-3 pt-0.5">
              {row.lines.map((lineWidth, lineIndex) => (
                <div
                  key={lineIndex}
                  className={cn([
                    "h-2.5 rounded-full bg-neutral-100",
                    "animate-pulse",
                    lineWidth,
                  ])}
                />
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function BatchProgressTimeline({
  sessionId,
  screen,
}: {
  sessionId: string;
  screen: Extract<
    ReturnType<typeof useTranscriptScreen>,
    { kind: "running_batch" }
  >;
}) {
  const stopTranscription = useListener((state) => state.stopTranscription);
  const handleStop = useCallback(() => {
    void stopTranscription(sessionId);
  }, [sessionId, stopTranscription]);
  const phaseLabel =
    screen.phase === "importing" ? "Importing" : "Transcribing";
  const canStopTranscription = screen.phase !== "importing";
  const progress = Math.max(0, Math.min(screen.percentage ?? 0, 1));
  const progressText =
    typeof screen.percentage === "number" && screen.percentage > 0
      ? `${Math.round(screen.percentage * 100)}%`
      : "...";

  return (
    <AudioPlayer.TimelineShell
      leading={
        <div
          className={cn([
            "flex h-8 w-8 items-center justify-center rounded-full",
            "border border-neutral-200 bg-white shadow-xs",
            "shrink-0",
          ])}
        >
          <Spinner size={14} />
        </div>
      }
      meta={
        <AudioPlayer.TimelineMeta>
          <span>{progressText}</span>
          {canStopTranscription ? (
            <StopTranscriptionButton onClick={handleStop} />
          ) : null}
        </AudioPlayer.TimelineMeta>
      }
      main={
        <div className="flex h-[30px] items-center">
          <div className="relative h-2.5 w-full overflow-hidden rounded-full bg-neutral-200/80">
            <div
              className="absolute inset-y-0 left-0 rounded-full bg-neutral-400 transition-[width] duration-300 ease-out"
              style={{ width: `${Math.max(progress * 100, 8)}%` }}
            />
            <div className="absolute inset-0 flex items-center justify-center">
              <span className="px-2 text-[10px] font-medium tracking-[0.02em] text-neutral-500">
                {phaseLabel}
              </span>
            </div>
          </div>
        </div>
      }
    />
  );
}

function StopTranscriptionButton({
  onClick,
  compact = false,
}: {
  onClick: () => void;
  compact?: boolean;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className={cn([
            "text-neutral-500 hover:text-neutral-700",
            compact ? "h-5 w-5" : "h-6 w-6",
          ])}
          onClick={onClick}
          aria-label="Stop transcription"
        >
          <SquareIcon size={compact ? 9 : 10} className="fill-current" />
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom">
        <p>Stop transcription</p>
      </TooltipContent>
    </Tooltip>
  );
}

function TranscriptReadyPanel({
  sessionId,
  isExpanded,
  fillHeight,
}: {
  sessionId: string;
  isExpanded: boolean;
  fillHeight: boolean;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const regenerate = useRegenerateTranscript(sessionId);
  const { audioExists, deleteRecording, isDeletingRecording } =
    AudioPlayer.useAudioPlayer();

  if (!isExpanded) {
    return null;
  }

  return (
    <TranscriptCard fillHeight={fillHeight}>
      <div className="flex shrink-0 items-center justify-between px-3 py-1.5">
        <div className="flex items-center gap-1">
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                disabled
                className={cn([
                  "flex items-center gap-1 rounded px-1.5 py-0.5",
                  "text-[11px] font-medium text-neutral-300",
                  "cursor-not-allowed",
                ])}
              >
                <Pencil size={10} />
                Edit
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom">
              <p>Coming soon</p>
            </TooltipContent>
          </Tooltip>
          <button
            type="button"
            onClick={regenerate}
            className={cn([
              "flex items-center gap-1 rounded px-1.5 py-0.5",
              "text-[11px] font-medium text-neutral-500",
              "transition-colors hover:bg-neutral-200/60 hover:text-neutral-700",
            ])}
          >
            <RefreshCw size={10} />
            Regenerate
          </button>
        </div>
        {audioExists ? (
          <button
            type="button"
            onClick={() => void deleteRecording()}
            disabled={isDeletingRecording}
            className={cn([
              "flex items-center gap-1 rounded px-1.5 py-0.5",
              "text-[11px] font-medium text-red-600",
              "transition-colors hover:bg-red-50 hover:text-red-700",
              "disabled:cursor-not-allowed disabled:text-red-300",
            ])}
          >
            {isDeletingRecording ? (
              <Loader2Icon size={10} className="animate-spin" />
            ) : (
              <TrashIcon size={10} />
            )}
            {isDeletingRecording ? "Deleting..." : "Delete recording"}
          </button>
        ) : null}
      </div>

      <TranscriptScrollArea fillHeight={fillHeight}>
        <Transcript sessionId={sessionId} scrollRef={scrollRef} />
      </TranscriptScrollArea>
    </TranscriptCard>
  );
}

function TranscriptEmptyPanel({
  sessionId,
  hasAudio,
  isExpanded,
  fillHeight,
}: {
  sessionId: string;
  hasAudio: boolean;
  isExpanded: boolean;
  fillHeight: boolean;
}) {
  const screen = useTranscriptScreen({ sessionId });
  const { uploadAudio } = useUploadFile(sessionId);
  const regenerate = useRegenerateTranscript(sessionId);

  const error = screen.kind === "empty" ? screen.error : null;

  if (!isExpanded) {
    return null;
  }

  return (
    <TranscriptCard fillHeight={fillHeight}>
      <div className="flex min-h-0 flex-1 items-center justify-between px-4 py-3">
        {error ? (
          <span className="text-xs text-red-500">{error}</span>
        ) : (
          <span className="text-xs text-neutral-400">No transcript yet</span>
        )}

        <div className="flex items-center gap-1.5">
          {hasAudio && (
            <Button
              variant="ghost"
              size="sm"
              className="h-7 gap-1.5 text-xs text-neutral-500"
              onClick={regenerate}
            >
              <RefreshCw size={12} />
              Generate
            </Button>
          )}
          <Button
            variant="ghost"
            size="sm"
            className="h-7 text-xs text-neutral-500"
            onClick={uploadAudio}
          >
            Upload audio
          </Button>
        </div>
      </div>
    </TranscriptCard>
  );
}

function TranscriptScrollArea({
  children,
  fillHeight,
}: {
  children: ReactNode;
  fillHeight: boolean;
}) {
  return (
    <div
      className={cn([
        "overflow-y-auto px-3",
        fillHeight ? "min-h-0 flex-1" : "h-[300px]",
      ])}
    >
      {children}
    </div>
  );
}

function TranscriptCard({
  children,
  fillHeight = false,
}: {
  children: ReactNode;
  fillHeight?: boolean;
}) {
  return (
    <div
      className={cn([
        "overflow-hidden rounded-b-xl border-x border-b border-neutral-200 bg-white",
        fillHeight ? "flex h-full min-h-0 flex-col" : "min-h-[96px]",
      ])}
    >
      {children}
    </div>
  );
}
