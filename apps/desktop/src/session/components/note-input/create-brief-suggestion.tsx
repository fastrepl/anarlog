import { useLingui } from "@lingui/react/macro";
import { Sparkle } from "@phosphor-icons/react";

import { cn } from "@anlg/utils";

import {
  type MemoBriefEditor,
  useCreatePreMeetingBrief,
} from "~/session/hooks/useCreatePreMeetingBrief";
import { useListener } from "~/stt/contexts";

const noop = () => {};

export function CreateBriefSuggestion({
  sessionId,
  getMemoEditor,
}: {
  sessionId: string;
  getMemoEditor: () => MemoBriefEditor | null;
}) {
  const { t } = useLingui();
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const { visible, isGenerating, createBrief } = useCreatePreMeetingBrief({
    sessionId,
    sessionMode,
    isMemoView: true,
    onSwitchToMemos: noop,
    getMemoEditor,
  });

  if (!visible) {
    return null;
  }

  const label = isGenerating
    ? t`Creating brief...`
    : t`Want me to create a brief to help you prepare?`;

  return (
    <div className="mb-6">
      <p className="text-muted-foreground flex h-8 items-center text-xs">
        {t`Prepare for this meeting`}
      </p>
      <button
        type="button"
        aria-label={label}
        disabled={isGenerating}
        onClick={createBrief}
        className={cn([
          "hover:bg-accent focus-visible:bg-accent pointer-events-auto -ml-2 flex h-8 w-fit max-w-full items-center gap-2 rounded-md px-2 text-left",
          "text-muted-foreground hover:text-foreground focus-visible:text-foreground transition-colors focus-visible:outline-hidden",
          isGenerating && "cursor-default opacity-60 hover:bg-transparent",
        ])}
      >
        <Sparkle
          aria-hidden
          className={cn(["size-4 shrink-0", isGenerating && "animate-pulse"])}
        />
        <span
          className={cn([
            "min-w-0 truncate text-sm font-medium",
            isGenerating && "animate-pulse",
          ])}
        >
          {label}
        </span>
      </button>
    </div>
  );
}
