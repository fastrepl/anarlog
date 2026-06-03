import {
  CopyIcon,
  Loader2Icon,
  Pencil,
  RefreshCw,
  SquareIcon,
  TrashIcon,
} from "lucide-react";
import { type ReactNode, useCallback, useMemo, useRef } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { Button } from "@hypr/ui/components/ui/button";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn, format, safeParseDate } from "@hypr/utils";

import * as AudioPlayer from "~/audio-player";
import { extractPlainText } from "~/search/contexts/engine/utils";
import { getEnhancerService } from "~/services/enhancer";
import { Transcript } from "~/session/components/note-input/transcript";
import {
  formatTranscriptExportSegments,
  useTranscriptExportSegments,
} from "~/session/components/note-input/transcript/export-data";
import { useTranscriptScreen } from "~/session/components/note-input/transcript/state";
import { getSessionEvent } from "~/session/utils";
import { showTransientToast } from "~/sidebar/toast/transient";
import * as main from "~/store/tinybase/store/main";
import { useListener } from "~/stt/contexts";
import { isStoppedTranscriptionError, useRunBatch } from "~/stt/useRunBatch";

export type PostSessionTab = "transcript" | "past_notes";

export type PastSessionNote = {
  sessionId: string;
  title: string;
  dateLabel: string;
  summary: string;
};

type MainStore = NonNullable<ReturnType<typeof main.UI.useStore>>;

export function PostSessionAccessory({
  sessionId,
  hasAudio,
  hasTranscript,
  isTranscriptExpanded,
  activeTab = "transcript",
  pastNotes = [],
  fillHeight = false,
}: {
  sessionId: string;
  hasAudio: boolean;
  hasTranscript: boolean;
  isTranscriptExpanded: boolean;
  activeTab?: PostSessionTab;
  pastNotes?: PastSessionNote[];
  fillHeight?: boolean;
}) {
  const screen = useTranscriptScreen({ sessionId });
  const isBatching = screen.kind === "running_batch";
  const effectiveActiveTab =
    activeTab === "past_notes" && pastNotes.length > 0
      ? "past_notes"
      : "transcript";
  const shouldFillExpandedPanel =
    fillHeight &&
    (effectiveActiveTab === "past_notes" || hasTranscript || isBatching);
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
            shouldFillExpandedPanel
              ? "min-h-[114px] flex-1 overflow-hidden"
              : "shrink-0",
          ])}
        >
          {effectiveActiveTab === "past_notes" ? (
            <PastNotesPanel
              notes={pastNotes}
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

export function usePastSessionNotes(sessionId: string): PastSessionNote[] {
  const store = main.UI.useStore(main.STORE_ID);
  const sessionsTable = main.UI.useTable("sessions", main.STORE_ID);
  const participantsTable = main.UI.useTable(
    "mapping_session_participant",
    main.STORE_ID,
  );
  const enhancedNotesTable = main.UI.useTable("enhanced_notes", main.STORE_ID);
  const userId = main.UI.useValue("user_id", main.STORE_ID);

  return useMemo(() => {
    if (!store) {
      return [];
    }

    return buildPastSessionNotes(
      store,
      sessionId,
      typeof userId === "string" ? userId : null,
    );
  }, [
    store,
    sessionId,
    userId,
    sessionsTable,
    participantsTable,
    enhancedNotesTable,
  ]);
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

function PastNotesPanel({
  notes,
  fillHeight,
}: {
  notes: PastSessionNote[];
  fillHeight: boolean;
}) {
  return (
    <TranscriptCard fillHeight={fillHeight}>
      <div className="flex shrink-0 items-center justify-between px-3 py-1.5">
        <span className="text-xs font-medium text-neutral-500">Past notes</span>
      </div>

      <div
        className={cn([
          "min-h-0 overflow-y-auto px-4 pb-4",
          fillHeight ? "flex-1" : "max-h-[300px]",
        ])}
      >
        <div className="relative flex flex-col gap-4 pt-2">
          <div className="absolute top-2 bottom-0 left-[3px] w-px bg-neutral-200" />
          {notes.map((note) => (
            <div
              key={note.sessionId}
              className="relative grid grid-cols-1 pl-5"
            >
              <div className="absolute top-1.5 left-0 h-2 w-2 rounded-full border border-neutral-300 bg-white" />
              <div className="flex min-w-0 flex-col gap-1">
                <div className="flex min-w-0 items-baseline justify-between gap-3">
                  <span className="min-w-0 truncate text-xs font-medium text-neutral-700">
                    {note.title}
                  </span>
                  <span className="shrink-0 text-[11px] text-neutral-400">
                    {note.dateLabel}
                  </span>
                </div>
                <p className="line-clamp-3 text-xs leading-5 text-neutral-500">
                  {note.summary}
                </p>
              </div>
            </div>
          ))}
        </div>
      </div>
    </TranscriptCard>
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
            "flex h-7 w-7 items-center justify-center rounded-full",
            "border border-neutral-200 bg-white shadow-xs",
            "shrink-0",
          ])}
        >
          <Spinner size={12} />
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
        <div className="flex h-6 items-center">
          <div className="relative h-2 w-full overflow-hidden rounded-full bg-neutral-200/80">
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
            onClick={handleCopyTranscript}
            disabled={!canCopyTranscript}
            aria-label="Copy transcript"
            className={cn([
              "flex items-center gap-1 rounded-full px-1.5 py-0.5",
              "text-[11px] font-medium text-neutral-500",
              "transition-colors hover:bg-neutral-200/60 hover:text-neutral-700",
              "disabled:cursor-not-allowed disabled:text-neutral-300",
              "disabled:hover:bg-transparent disabled:hover:text-neutral-300",
            ])}
          >
            <CopyIcon size={10} />
            {isTranscriptLoading ? "Loading..." : "Copy"}
          </button>
          <button
            type="button"
            onClick={regenerate}
            className={cn([
              "flex items-center gap-1 rounded-full px-1.5 py-0.5",
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
              "flex items-center gap-1 rounded-full px-1.5 py-0.5",
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
        "overflow-hidden rounded-b-xl border border-neutral-200 bg-white",
        fillHeight && "flex h-full flex-col",
        fillHeight && reserveMinHeight && "min-h-[114px]",
        !fillHeight && reserveMinHeight && "min-h-[96px]",
      ])}
    >
      {children}
    </div>
  );
}

const MAX_PAST_NOTES = 8;
const MAX_SUMMARY_LENGTH = 220;
const SUMMARY_TARGET_LENGTH = 150;
const SPACE_REGEX = /\s+/g;

export function buildPastSessionNotes(
  store: MainStore,
  sessionId: string,
  userId: string | null,
): PastSessionNote[] {
  const currentSession = store.getRow("sessions", sessionId);
  if (!currentSession) {
    return [];
  }

  const currentParticipantIds = getSessionParticipantIds(
    store,
    sessionId,
    userId,
  );
  const currentEvent = getSessionEvent(currentSession);
  const currentSeriesId = getRecurrenceSeriesId(currentEvent);
  if (!currentSeriesId && currentParticipantIds.size === 0) {
    return [];
  }

  const currentTimestamp = getSessionTimestamp(currentSession);
  const notes: Array<PastSessionNote & { dateMs: number }> = [];

  store.forEachRow("sessions", (candidateSessionId, _forEachCell) => {
    if (candidateSessionId === sessionId) {
      return;
    }

    const candidateSession = store.getRow("sessions", candidateSessionId);
    if (!candidateSession) {
      return;
    }

    const candidateTimestamp = getSessionTimestamp(candidateSession);
    if (
      currentTimestamp > 0 &&
      candidateTimestamp > 0 &&
      candidateTimestamp >= currentTimestamp
    ) {
      return;
    }

    const candidateEvent = getSessionEvent(candidateSession);
    const candidateParticipantIds = getSessionParticipantIds(
      store,
      candidateSessionId,
      userId,
    );
    if (
      !isRelatedPastSession({
        currentParticipantIds,
        currentSeriesId,
        candidateParticipantIds,
        candidateSeriesId: getRecurrenceSeriesId(candidateEvent),
      })
    ) {
      return;
    }

    const summary = getSessionNoteSummary(store, candidateSessionId);
    if (!summary) {
      return;
    }

    notes.push({
      sessionId: candidateSessionId,
      title: getSessionTitle(candidateSession),
      dateLabel: formatSessionDate(candidateSession),
      summary,
      dateMs: candidateTimestamp,
    });
  });

  return notes
    .sort((a, b) => b.dateMs - a.dateMs)
    .slice(0, MAX_PAST_NOTES)
    .map(({ dateMs: _dateMs, ...note }) => note);
}

function getSessionParticipantIds(
  store: MainStore,
  sessionId: string,
  userId: string | null,
): Set<string> {
  const participantIds = new Set<string>();

  store.forEachRow("mapping_session_participant", (mappingId, _forEachCell) => {
    const mapping = store.getRow("mapping_session_participant", mappingId);
    if (
      mapping.session_id !== sessionId ||
      mapping.source === "excluded" ||
      !mapping.human_id
    ) {
      return;
    }

    const ownerUserId =
      typeof mapping.user_id === "string" && mapping.user_id.trim()
        ? mapping.user_id
        : null;
    const isCurrentUser =
      (userId && mapping.human_id === userId) ||
      (!userId && ownerUserId && mapping.human_id === ownerUserId);
    if (!isCurrentUser) {
      participantIds.add(mapping.human_id);
    }
  });

  return participantIds;
}

function isRelatedPastSession({
  currentParticipantIds,
  currentSeriesId,
  candidateParticipantIds,
  candidateSeriesId,
}: {
  currentParticipantIds: Set<string>;
  currentSeriesId: string | null;
  candidateParticipantIds: Set<string>;
  candidateSeriesId: string | null;
}) {
  if (currentSeriesId && candidateSeriesId === currentSeriesId) {
    return true;
  }

  if (currentParticipantIds.size === 0) {
    return false;
  }

  for (const participantId of currentParticipantIds) {
    if (!candidateParticipantIds.has(participantId)) {
      return false;
    }
  }

  return true;
}

function getSessionNoteSummary(store: MainStore, sessionId: string): string {
  const enhancedNotes: Array<{ content: string; position: number }> = [];

  store.forEachRow("enhanced_notes", (noteId, _forEachCell) => {
    const note = store.getRow("enhanced_notes", noteId);
    if (note.session_id !== sessionId || !note.content?.trim()) {
      return;
    }

    enhancedNotes.push({
      content: note.content,
      position: typeof note.position === "number" ? note.position : 0,
    });
  });

  enhancedNotes.sort((a, b) => a.position - b.position);
  for (const note of enhancedNotes) {
    const summary = toConciseSummary(note.content);
    if (summary) {
      return summary;
    }
  }

  const rawMd = store.getCell("sessions", sessionId, "raw_md");
  return toConciseSummary(rawMd);
}

function toConciseSummary(value: unknown): string {
  const cleaned = cleanSummaryText(extractPlainText(value));
  if (!cleaned) {
    return "";
  }

  const sentences = cleaned.match(/[^.!?]+(?:[.!?]+|$)/g) ?? [cleaned];
  let summary = "";

  for (const sentence of sentences) {
    const trimmed = sentence.trim();
    if (!trimmed) {
      continue;
    }

    const next = summary ? `${summary} ${trimmed}` : trimmed;
    if (summary && next.length > SUMMARY_TARGET_LENGTH) {
      break;
    }

    summary = next;
    if (summary.length >= SUMMARY_TARGET_LENGTH) {
      break;
    }
  }

  return truncateAtWord(summary || cleaned, MAX_SUMMARY_LENGTH);
}

function cleanSummaryText(text: string): string {
  return text
    .replace(/!\[[^\]]*]\([^)]+\)/g, "")
    .replace(/\[([^\]]+)]\([^)]+\)/g, "$1")
    .replace(/[`*_~>#]/g, "")
    .replace(/(^|\s)([-+]|[0-9]+[.)])\s+/g, " ")
    .replace(SPACE_REGEX, " ")
    .trim();
}

function truncateAtWord(text: string, maxLength: number): string {
  if (text.length <= maxLength) {
    return text;
  }

  const slice = text.slice(0, maxLength + 1);
  const lastSpace = slice.lastIndexOf(" ");
  const end = lastSpace > maxLength * 0.6 ? lastSpace : maxLength;
  return `${slice.slice(0, end).trim()}...`;
}

function getSessionTitle(session: { title?: string }): string {
  return session.title?.trim() || "Untitled";
}

function getRecurrenceSeriesId(
  event: ReturnType<typeof getSessionEvent>,
): string | null {
  const seriesId = event?.recurrence_series_id?.trim();
  return seriesId || null;
}

function getSessionTimestamp(session: {
  created_at?: string;
  event_json?: string;
}): number {
  const event = getSessionEvent(session);
  const value = event?.started_at || session.created_at;
  if (!value) {
    return 0;
  }

  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp) ? 0 : timestamp;
}

function formatSessionDate(session: {
  created_at?: string;
  event_json?: string;
}): string {
  const event = getSessionEvent(session);
  const parsed = safeParseDate(event?.started_at || session.created_at);
  return parsed ? format(parsed, "MMM d, yyyy") : "";
}
