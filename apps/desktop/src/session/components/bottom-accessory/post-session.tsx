import {
  CopyIcon,
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
import type { PastSessionNote } from "~/session/components/bottom-accessory/past-notes";
import { Transcript } from "~/session/components/note-input/transcript";
import {
  formatTranscriptExportSegments,
  useTranscriptExportSegments,
} from "~/session/components/note-input/transcript/export-data";
import { useTranscriptScreen } from "~/session/components/note-input/transcript/state";
import { showTransientToast } from "~/sidebar/toast/transient";
import { useListener } from "~/stt/contexts";
import { isStoppedTranscriptionError, useRunBatch } from "~/stt/useRunBatch";

export type PostSessionTab = "transcript" | "insights";

const MAX_COMPILED_INSIGHTS = 12;

export function PostSessionAccessory({
  sessionId,
  hasAudio,
  hasTranscript,
  isTranscriptExpanded,
  activeTab = "transcript",
  pastNotes = [],
  suppressTranscriptPanel = false,
  onRegenerateInsights,
  fillHeight = false,
}: {
  sessionId: string;
  hasAudio: boolean;
  hasTranscript: boolean;
  isTranscriptExpanded: boolean;
  activeTab?: PostSessionTab;
  pastNotes?: PastSessionNote[];
  suppressTranscriptPanel?: boolean;
  onRegenerateInsights?: () => void;
  fillHeight?: boolean;
}) {
  const screen = useTranscriptScreen({ sessionId });
  const isBatching = screen.kind === "running_batch";
  const effectiveActiveTab =
    activeTab === "insights" && pastNotes.length > 0
      ? "insights"
      : "transcript";
  const shouldFillExpandedPanel =
    fillHeight &&
    (effectiveActiveTab === "insights" || hasTranscript || isBatching);
  const timeline = isBatching ? (
    <BatchProgressTimeline sessionId={sessionId} screen={screen} />
  ) : hasAudio && isTranscriptExpanded ? (
    <AudioPlayer.Timeline />
  ) : null;

  if (!isTranscriptExpanded && !timeline) {
    return null;
  }

  return (
    <div
      className={cn([
        "flex min-h-0 flex-col",
        shouldFillExpandedPanel && "h-full overflow-hidden",
      ])}
    >
      {isTranscriptExpanded && !suppressTranscriptPanel ? (
        <div
          className={cn([
            shouldFillExpandedPanel
              ? "min-h-[114px] flex-1 overflow-hidden"
              : "shrink-0",
          ])}
        >
          {effectiveActiveTab === "insights" ? (
            <InsightsPanel
              notes={pastNotes}
              onRegenerateInsights={onRegenerateInsights}
              fillHeight={shouldFillExpandedPanel}
            />
          ) : (
            <TranscriptPanel
              sessionId={sessionId}
              screen={screen}
              hasAudio={hasAudio}
              hasTranscript={hasTranscript}
              isExpanded={isTranscriptExpanded}
              fillHeight={shouldFillExpandedPanel}
            />
          )}
        </div>
      ) : null}
      {timeline ? (
        <TimelineSlot flushTop={!isTranscriptExpanded}>{timeline}</TimelineSlot>
      ) : null}
    </div>
  );
}

function TimelineSlot({
  children,
  flushTop = false,
}: {
  children: ReactNode;
  flushTop?: boolean;
}) {
  return (
    <div
      className={cn([
        "flex h-10 w-full shrink-0 items-center",
        flushTop && "-mt-1.5",
      ])}
    >
      {children}
    </div>
  );
}

function InsightsPanel({
  notes,
  onRegenerateInsights,
  fillHeight,
}: {
  notes: PastSessionNote[];
  onRegenerateInsights?: () => void;
  fillHeight: boolean;
}) {
  const participantNames = getCompiledParticipantNames(notes);
  const insightFacts = getCompiledInsightFacts(notes);
  const isGenerating = notes.some((note) => note.isGenerating);
  const isRegenerateDisabled =
    notes.length === 0 ||
    notes.some((note) => note.isGenerating || note.isRegenerateDisabled);

  return (
    <TranscriptCard fillHeight={fillHeight}>
      <div className="flex shrink-0 items-center justify-between px-3 py-1.5">
        <span className="text-muted-foreground text-xs font-medium">
          Insights
        </span>
        {onRegenerateInsights ? (
          <RegenerateInsightsButton
            isDisabled={isRegenerateDisabled}
            isGenerating={isGenerating}
            onClick={onRegenerateInsights}
          />
        ) : null}
      </div>

      <div
        className={cn([
          "min-h-0 overflow-y-auto px-4 pb-4",
          fillHeight ? "flex-1" : "max-h-[300px]",
        ])}
      >
        <div className="flex min-w-0 flex-col gap-3 pt-2">
          {participantNames.length > 0 ? (
            <div className="flex min-w-0 flex-wrap items-center gap-1.5">
              <span className="text-muted-foreground mr-0.5 text-[11px] font-medium">
                With
              </span>
              {participantNames.map((participantName) => (
                <span
                  key={participantName}
                  className="border-border bg-accent/35 text-muted-foreground max-w-full truncate rounded-full border px-2 py-0.5 text-[11px] leading-4"
                >
                  {participantName}
                </span>
              ))}
            </div>
          ) : null}

          {insightFacts.length > 0 ? (
            <ul className="text-muted-foreground min-w-0 list-disc space-y-1.5 pr-1 pl-5 text-xs leading-5">
              {insightFacts.map((fact) => (
                <li key={fact.key} className="min-w-0 break-words">
                  {fact.text}
                </li>
              ))}
            </ul>
          ) : (
            <p className="text-muted-foreground text-xs leading-5">
              {isGenerating
                ? "Generating insights..."
                : "Insights will be generated when this tab opens."}
            </p>
          )}

          {insightFacts.length > 0 && isGenerating ? (
            <p className="text-muted-foreground text-xs leading-5">
              Updating insights...
            </p>
          ) : null}
        </div>
      </div>
    </TranscriptCard>
  );
}

function splitKeyFacts(content: string): string[] {
  return content
    .split("\n")
    .map((fact) =>
      fact
        .replace(/^[-*]\s+/, "")
        .replace(/^\d+[.)]\s+/, "")
        .trim(),
    )
    .filter(Boolean)
    .slice(0, 3);
}

function getCompiledParticipantNames(notes: PastSessionNote[]): string[] {
  const seen = new Set<string>();
  const names: string[] = [];

  for (const note of notes) {
    for (const participantName of note.participantNames ?? []) {
      const text = participantName.trim();
      const key = text.toLowerCase();
      if (!text || seen.has(key)) {
        continue;
      }

      seen.add(key);
      names.push(text);
    }
  }

  return names.sort((a, b) => a.localeCompare(b));
}

function getCompiledInsightFacts(
  notes: PastSessionNote[],
): Array<{ key: string; text: string }> {
  const seen = new Set<string>();
  const facts: Array<{ key: string; text: string }> = [];

  for (const note of notes) {
    if (!note.summary) {
      continue;
    }

    for (const fact of splitKeyFacts(note.summary)) {
      const key = fact.toLowerCase();
      if (seen.has(key)) {
        continue;
      }

      seen.add(key);
      facts.push({ key: `${note.sessionId}-${key}`, text: fact });
      if (facts.length >= MAX_COMPILED_INSIGHTS) {
        return facts;
      }
    }
  }

  return facts;
}

function RegenerateInsightsButton({
  isDisabled,
  isGenerating,
  onClick,
}: {
  isDisabled: boolean;
  isGenerating: boolean;
  onClick: () => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          aria-label="Regenerate insights"
          disabled={isDisabled}
          onClick={onClick}
          className={cn([
            "text-muted-foreground h-5 w-5 shrink-0",
            "hover:bg-accent/60 hover:text-muted-foreground",
            "disabled:text-muted-foreground/70 disabled:cursor-not-allowed",
          ])}
        >
          {isGenerating ? (
            <Loader2Icon size={10} className="animate-spin" />
          ) : (
            <RefreshCw size={10} />
          )}
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom">
        <p>{isGenerating ? "Regenerating insights" : "Regenerate insights"}</p>
      </TooltipContent>
    </Tooltip>
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
    if (result.status === "error") {
      showTransientToast({
        id: `transcript-regenerate-audio-missing-${sessionId}`,
        description: "Recording not found. It may have been deleted.",
        variant: "error",
      });
      return;
    }

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
  const { phase } = screen;
  const phaseLabel = getBatchPhaseLabel(phase);
  const canStopTranscription = phase !== "importing";

  if (!isExpanded) {
    return null;
  }

  return (
    <TranscriptCard fillHeight={fillHeight}>
      <div className="flex shrink-0 items-center justify-between px-3 py-1.5">
        <span className="text-muted-foreground text-xs font-medium">
          Transcript
        </span>
        <div className="flex items-center gap-1 px-1 py-0.5">
          <BatchStatusControl
            onStop={canStopTranscription ? handleStop : undefined}
            compact
          />
          <span className="text-muted-foreground text-[11px]">
            {phaseLabel}
          </span>
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
                  "bg-accent/80 h-2.5 rounded-full",
                  "animate-pulse",
                  row.speaker,
                ])}
              />
              <div
                className={cn([
                  "bg-muted h-1.5 rounded-full",
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
                    "bg-muted h-2.5 rounded-full",
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
  const phaseLabel = getBatchPhaseLabel(screen.phase);
  const canStopTranscription = screen.phase !== "importing";

  return (
    <AudioPlayer.TimelineShell
      leading={
        <BatchStatusControl
          onStop={canStopTranscription ? handleStop : undefined}
        />
      }
      main={
        <div className="flex h-6 items-center justify-center">
          <span className="text-muted-foreground text-[11px] font-medium">
            {phaseLabel}
          </span>
        </div>
      }
    />
  );
}

function getBatchPhaseLabel(phase?: "importing" | "transcribing") {
  return phase === "importing" ? "Uploading..." : "Transcribing...";
}

function BatchStatusControl({
  onStop,
  compact,
}: {
  onStop?: () => void;
  compact?: boolean;
}) {
  const sizeClassName = compact ? "h-5 w-5" : "h-7 w-7";
  const spinnerSize = compact ? 10 : 12;
  const iconSize = compact ? 9 : 10;

  if (!onStop) {
    return (
      <div
        className={cn([
          "flex items-center justify-center rounded-full",
          "border-border bg-card border shadow-xs",
          "shrink-0",
          sizeClassName,
        ])}
      >
        <Spinner size={spinnerSize} />
      </div>
    );
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className={cn([
            "group rounded-full",
            "border-border bg-card border shadow-xs",
            "text-muted-foreground hover:bg-accent hover:text-muted-foreground",
            "shrink-0 transition-colors",
            sizeClassName,
          ])}
          onClick={onStop}
          aria-label="Stop transcription"
        >
          <span className="group-hover:hidden">
            <Spinner size={spinnerSize} />
          </span>
          <SquareIcon
            size={iconSize}
            className="hidden fill-current group-hover:block"
          />
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
  const { data: transcriptSegments, isLoading: isTranscriptLoading } =
    useTranscriptExportSegments(sessionId);
  const { audioExists, deleteRecording, isDeletingRecording } =
    AudioPlayer.useAudioPlayer();
  const transcriptText = formatTranscriptExportSegments(transcriptSegments);
  const canCopyTranscript = transcriptText.length > 0 && !isTranscriptLoading;
  const handleCopyTranscript = useCallback(() => {
    if (!canCopyTranscript) {
      return;
    }

    void copyTranscriptToClipboard(transcriptText);
  }, [canCopyTranscript, transcriptText]);

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
                  "flex items-center gap-1 rounded-full px-1.5 py-0.5",
                  "text-muted-foreground/70 text-[11px] font-medium",
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
            onClick={handleCopyTranscript}
            disabled={!canCopyTranscript}
            aria-label="Copy transcript"
            className={cn([
              "flex items-center gap-1 rounded-full px-1.5 py-0.5",
              "text-muted-foreground text-[11px] font-medium",
              "hover:bg-accent/60 hover:text-muted-foreground transition-colors",
              "disabled:text-muted-foreground/70 disabled:cursor-not-allowed",
              "disabled:hover:text-muted-foreground/70 disabled:hover:bg-transparent",
            ])}
          >
            <CopyIcon size={10} />
            {isTranscriptLoading ? "Loading..." : "Copy"}
          </button>
          {audioExists ? (
            <button
              type="button"
              onClick={regenerate}
              className={cn([
                "flex items-center gap-1 rounded-full px-1.5 py-0.5",
                "text-muted-foreground text-[11px] font-medium",
                "hover:bg-accent/60 hover:text-muted-foreground transition-colors",
              ])}
            >
              <RefreshCw size={10} />
              Regenerate
            </button>
          ) : null}
        </div>
        {audioExists ? (
          <button
            type="button"
            onClick={() => void deleteRecording()}
            disabled={isDeletingRecording}
            className={cn([
              "flex items-center gap-1 rounded-full px-1.5 py-0.5",
              "text-[11px] font-medium text-red-600 dark:text-red-400",
              "transition-colors hover:bg-red-50 hover:text-red-700 dark:hover:bg-red-950/50 dark:hover:text-red-300",
              "disabled:cursor-not-allowed disabled:text-red-300 dark:disabled:text-red-500/60",
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

async function copyTranscriptToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    showTransientToast({
      id: "transcript-copy-success",
      description: "Transcript copied to clipboard",
    });
  } catch (error) {
    console.error("Failed to copy transcript", error);
    showTransientToast({
      id: "transcript-copy-error",
      description: "Failed to copy transcript",
      variant: "error",
    });
  }
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
  const regenerate = useRegenerateTranscript(sessionId);

  const error = screen.kind === "empty" ? screen.error : null;

  if (!isExpanded) {
    return null;
  }

  return (
    <TranscriptCard fillHeight={fillHeight} reserveMinHeight={false}>
      <div className="flex min-h-0 flex-1 items-center justify-between px-4 py-3">
        {error ? (
          <span className="text-xs text-red-500">{error}</span>
        ) : (
          <span className="text-muted-foreground text-xs">
            No transcript yet
          </span>
        )}

        <div className="flex items-center gap-1.5">
          {hasAudio && (
            <Button
              variant="ghost"
              size="sm"
              className="text-muted-foreground h-7 gap-1.5 text-xs"
              onClick={regenerate}
            >
              <RefreshCw size={12} />
              Regenerate
            </Button>
          )}
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
  reserveMinHeight = true,
}: {
  children: ReactNode;
  fillHeight?: boolean;
  reserveMinHeight?: boolean;
}) {
  return (
    <div
      data-session-transcript-card
      className={cn([
        "border-border bg-card overflow-hidden rounded-b-xl border",
        fillHeight && "flex h-full flex-col",
        fillHeight && reserveMinHeight && "min-h-[114px]",
        !fillHeight && reserveMinHeight && "min-h-[96px]",
      ])}
    >
      {children}
    </div>
  );
}
