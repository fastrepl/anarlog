import { useLingui } from "@lingui/react/macro";
import { Loader2Icon, RefreshCwIcon } from "lucide-react";
import { useMemo } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import {
  type PastSessionNote,
  usePastSessionNotes,
} from "~/session/insights/past-notes";

const MAX_COMPILED_INSIGHTS = 12;

export function Insights({ sessionId }: { sessionId: string }) {
  const pastNotes = usePastSessionNotes(sessionId);
  const insightFacts = useMemo(
    () => getCompiledInsightFacts(pastNotes.notes),
    [pastNotes.notes],
  );
  const isRegenerateDisabled =
    !pastNotes.canGenerate ||
    pastNotes.notes.length === 0 ||
    pastNotes.notes.some(
      (note) => note.isGenerating || note.isRegenerateDisabled,
    );

  return (
    <div data-session-insights className="relative h-full min-h-0">
      {pastNotes.notes.length > 0 ? (
        <div className="absolute top-2 right-1 z-10">
          <RegenerateInsightsButton
            isDisabled={isRegenerateDisabled}
            isGenerating={pastNotes.isGenerating}
            onClick={pastNotes.regenerateAll}
          />
        </div>
      ) : null}

      <div
        className={cn([
          "scroll-fade-y h-full overflow-y-auto py-3 pl-2",
          pastNotes.notes.length > 0 ? "pr-9" : "pr-2",
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
              {pastNotes.isGenerating
                ? "Generating insights..."
                : "No insights yet."}
            </p>
          )}

          {insightFacts.length > 0 && pastNotes.isGenerating ? (
            <p className="text-muted-foreground text-xs leading-5">
              Updating insights...
            </p>
          ) : null}
        </div>
      </div>
    </div>
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

export function getCompiledInsightFacts(
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
            "text-muted-foreground h-6 w-6 shrink-0 rounded-full",
            "hover:bg-accent/70 hover:text-foreground",
            "disabled:text-muted-foreground/70 disabled:cursor-not-allowed",
          ])}
        >
          {isGenerating ? (
            <Loader2Icon size={12} className="animate-spin" />
          ) : (
            <RefreshCwIcon size={12} />
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
