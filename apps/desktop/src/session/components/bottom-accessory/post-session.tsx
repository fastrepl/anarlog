import { Pencil, RefreshCw, SquareIcon } from "lucide-react";
import { useCallback, useRef } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { Button } from "@hypr/ui/components/ui/button";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { ExpandToggle } from "./expand-toggle";

import * as AudioPlayer from "~/audio-player";
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
  onToggleTranscript,
}: {
  sessionId: string;
  hasAudio: boolean;
  hasTranscript: boolean;
  isTranscriptExpanded: boolean;
  onToggleTranscript: () => void;
}) {
  const screen = useTranscriptScreen({ sessionId });

  return (
    <div className="flex flex-col gap-1">
      <TranscriptPanel
        sessionId={sessionId}
        screen={screen}
        hasTranscript={hasTranscript}
        isExpanded={isTranscriptExpanded}
        onToggleExpand={onToggleTranscript}
      />
      {screen.kind === "running_batch" ? (
        <BatchProgressTimeline sessionId={sessionId} screen={screen} />
      ) : hasAudio ? (
        <AudioPlayer.Timeline />
      ) : null}
    </div>
  );
}

function TranscriptPanel({
  sessionId,
  screen,
  hasTranscript,
  isExpanded,
  onToggleExpand,
}: {
  sessionId: string;
  screen: ReturnType<typeof useTranscriptScreen>;
  hasTranscript: boolean;
  isExpanded: boolean;
  onToggleExpand: () => void;
}) {
  if (screen.kind === "running_batch") {
    return (
      <BatchingTranscriptPanel
        sessionId={sessionId}
        screen={screen}
        hasTranscript={hasTranscript}
        isExpanded={isExpanded}
        onToggleExpand={onToggleExpand}
      />
    );
  }

  if (hasTranscript) {
    return (
      <TranscriptReadyPanel
        sessionId={sessionId}
        isExpanded={isExpanded}
        onToggleExpand={onToggleExpand}
      />
    );
  }

  return (
    <TranscriptEmptyPanel
      sessionId={sessionId}
      isExpanded={isExpanded}
      onToggleExpand={onToggleExpand}
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
  hasTranscript,
  isExpanded,
  onToggleExpand,
}: {
  sessionId: string;
  screen: {
    kind: "running_batch";
    percentage?: number;
    phase?: "importing" | "transcribing";
  };
  hasTranscript: boolean;
  isExpanded: boolean;
  onToggleExpand: () => void;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const stopTranscription = useListener((state) => state.stopTranscription);
  const handleStop = useCallback(() => {
    void stopTranscription(sessionId);
  }, [sessionId, stopTranscription]);
  const { percentage, phase } = screen;
  const phaseLabel = phase === "importing" ? "Importing..." : "Transcribing...";
  const canStopTranscription = phase !== "importing";

  return (
    <div className="relative w-full pt-1 select-none">
      <ExpandToggle
        isExpanded={isExpanded}
        onToggle={onToggleExpand}
        label="Transcript"
      />

      {isExpanded && (
        <div className="rounded-xl bg-neutral-50">
          <div className="flex items-center justify-between rounded-t-xl border-b border-neutral-200 bg-neutral-100 px-3 py-1.5">
            <span className="text-xs font-medium text-neutral-500">
              Transcript
            </span>
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

          {hasTranscript ? (
            <div className="h-[300px] overflow-y-auto px-3">
              <Transcript sessionId={sessionId} scrollRef={scrollRef} />
            </div>
          ) : (
            <div className="flex h-[120px] flex-col items-center justify-center gap-2">
              <Spinner size={24} />
              {typeof percentage === "number" && percentage > 0 && (
                <p className="text-xl font-medium text-neutral-500 tabular-nums">
                  {Math.round(percentage * 100)}%
                </p>
              )}
              <p className="text-sm text-neutral-400">{phaseLabel}</p>
            </div>
          )}
        </div>
      )}
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
  onToggleExpand,
}: {
  sessionId: string;
  isExpanded: boolean;
  onToggleExpand: () => void;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const regenerate = useRegenerateTranscript(sessionId);

  return (
    <div className="relative w-full pt-1 select-none">
      <ExpandToggle
        isExpanded={isExpanded}
        onToggle={onToggleExpand}
        label="Transcript"
      />

      {isExpanded && (
        <div className="rounded-xl bg-neutral-50">
          <div className="flex items-center justify-between rounded-t-xl border-b border-neutral-200 bg-neutral-100 px-3 py-1.5">
            <span className="text-xs font-medium text-neutral-500">
              Transcript
            </span>
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
          </div>

          <div className="h-[300px] overflow-y-auto px-3">
            <Transcript sessionId={sessionId} scrollRef={scrollRef} />
          </div>
        </div>
      )}
    </div>
  );
}

function TranscriptEmptyPanel({
  sessionId,
  isExpanded,
  onToggleExpand,
}: {
  sessionId: string;
  isExpanded: boolean;
  onToggleExpand: () => void;
}) {
  const screen = useTranscriptScreen({ sessionId });
  const { uploadAudio } = useUploadFile(sessionId);
  const regenerate = useRegenerateTranscript(sessionId);

  const error = screen.kind === "empty" ? screen.error : null;
  const hasAudio = screen.kind === "empty" ? screen.hasAudio : false;

  return (
    <div className="relative w-full pt-1 select-none">
      <ExpandToggle
        isExpanded={isExpanded}
        onToggle={onToggleExpand}
        label="Transcript"
      />

      {isExpanded && (
        <div className="rounded-xl bg-neutral-50">
          <div className="flex items-center justify-between px-4 py-3">
            {error ? (
              <span className="text-xs text-red-500">{error}</span>
            ) : (
              <span className="text-xs text-neutral-400">
                No transcript yet
              </span>
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
        </div>
      )}
    </div>
  );
}
