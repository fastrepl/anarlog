import { Trans, useLingui } from "@lingui/react/macro";
import { Loader2Icon, RefreshCw, SquareIcon } from "lucide-react";
import { type ReactNode, useCallback } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import * as AudioPlayer from "~/audio-player";
import type { PastSessionNote } from "~/session/components/bottom-accessory/past-notes";
import { useTranscriptScreen } from "~/session/components/note-input/transcript/state";
import { useListener } from "~/stt/contexts";

export type PostSessionTab = "transcript" | "insights";

const MAX_COMPILED_INSIGHTS = 12;

export function PostSessionAccessory({
  sessionId,
  isTranscriptExpanded,
  activeTab = "transcript",
  pastNotes = [],
  onRegenerateInsights,
  fillHeight = false,
}: {
  sessionId: string;
  isTranscriptExpanded: boolean;
  activeTab?: PostSessionTab;
  pastNotes?: PastSessionNote[];
  onRegenerateInsights?: () => void;
  fillHeight?: boolean;
}) {
  const screen = useTranscriptScreen({ sessionId });
  const isBatching = screen.kind === "running_batch";
  const showInsightsPanel =
    activeTab === "insights" && pastNotes.length > 0 && isTranscriptExpanded;
  const shouldFillExpandedPanel =
    fillHeight && (showInsightsPanel || isBatching);
  const timeline = isBatching ? (
    <BatchProgressTimeline sessionId={sessionId} screen={screen} />
  ) : null;

  if (!showInsightsPanel && !timeline) {
    return null;
  }

  return (
    <div
      className={cn([
        "flex min-h-0 flex-col",
        shouldFillExpandedPanel && "h-full overflow-hidden",
      ])}
    >
      {showInsightsPanel ? (
        <div
          className={cn([
            shouldFillExpandedPanel
              ? "min-h-[114px] flex-1 overflow-hidden"
              : "shrink-0",
          ])}
        >
          <InsightsPanel
            notes={pastNotes}
            onRegenerateInsights={onRegenerateInsights}
            fillHeight={shouldFillExpandedPanel}
          />
        </div>
      ) : null}
      {timeline ? (
        <TimelineSlot flushTop={!showInsightsPanel}>{timeline}</TimelineSlot>
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
  const insightFacts = getCompiledInsightFacts(notes);
  const isGenerating = notes.some((note) => note.isGenerating);
  const isRegenerateDisabled =
    notes.length === 0 ||
    notes.some((note) => note.isGenerating || note.isRegenerateDisabled);

  return (
    <TranscriptCard fillHeight={fillHeight}>
      <div
        className={cn([
          "relative min-h-0",
          fillHeight && "flex flex-1 flex-col",
        ])}
      >
        {onRegenerateInsights ? (
          <div className="absolute top-2 right-3 z-10">
            <RegenerateInsightsButton
              isDisabled={isRegenerateDisabled}
              isGenerating={isGenerating}
              onClick={onRegenerateInsights}
            />
          </div>
        ) : null}

        <div
          className={cn([
            "min-h-0 overflow-y-auto py-3 pl-4",
            onRegenerateInsights ? "pr-10" : "pr-4",
            fillHeight ? "flex-1" : "max-h-[300px]",
          ])}
        >
          <div className="flex min-w-0 flex-col gap-2">
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
                {isGenerating ? "Generating insights..." : "No insights yet."}
              </p>
            )}

            {insightFacts.length > 0 && isGenerating ? (
              <p className="text-muted-foreground text-xs leading-5">
                Updating insights...
              </p>
            ) : null}
          </div>
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
  const { t } = useLingui();
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          aria-label={t`Regenerate insights`}
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
        <p>
          {isGenerating ? t`Regenerating insights` : t`Regenerate insights`}
        </p>
      </TooltipContent>
    </Tooltip>
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
  const { t } = useLingui();
  const stopTranscription = useListener((state) => state.stopTranscription);
  const handleStop = useCallback(() => {
    void stopTranscription(sessionId);
  }, [sessionId, stopTranscription]);
  const phaseLabel = getBatchPhaseLabel(t, screen.phase);
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

function getBatchPhaseLabel(
  t: ReturnType<typeof useLingui>["t"],
  phase?: "importing" | "transcribing",
) {
  return phase === "importing" ? t`Uploading...` : t`Transcribing...`;
}

function BatchStatusControl({
  onStop,
  compact,
}: {
  onStop?: () => void;
  compact?: boolean;
}) {
  const { t } = useLingui();
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
          aria-label={t`Stop transcription`}
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
        <p>
          <Trans>Stop transcription</Trans>
        </p>
      </TooltipContent>
    </Tooltip>
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
