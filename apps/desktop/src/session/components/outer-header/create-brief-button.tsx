import { useLingui } from "@lingui/react/macro";
import { Sparkle } from "@phosphor-icons/react";

import { cn } from "@anlg/utils";

import {
  type MemoBriefEditor,
  useCreatePreMeetingBrief,
} from "~/session/hooks/useCreatePreMeetingBrief";
import type { SessionMode } from "~/store/zustand/listener/general";

export function CreateBriefButton({
  sessionId,
  sessionMode,
  isMemoView,
  onSwitchToMemos,
  getMemoEditor,
}: {
  sessionId: string;
  sessionMode: SessionMode;
  isMemoView: boolean;
  onSwitchToMemos: () => void;
  getMemoEditor: () => MemoBriefEditor | null;
}) {
  const { t } = useLingui();
  const { visible, isGenerating, createBrief } = useCreatePreMeetingBrief({
    sessionId,
    sessionMode,
    isMemoView,
    onSwitchToMemos,
    getMemoEditor,
  });

  if (!visible) {
    return null;
  }

  const label = isGenerating ? t`Creating brief...` : t`Create brief`;

  return (
    <button
      type="button"
      data-tauri-drag-region="false"
      aria-label={label}
      title={label}
      disabled={isGenerating}
      onClick={createBrief}
      className={cn([
        "flex h-7 max-w-56 shrink-0 items-center gap-1.5 overflow-hidden rounded-full border px-2",
        "text-sm font-medium",
        "border-border bg-card text-foreground",
        "transition-colors",
        !isGenerating && "hover:bg-accent",
        isGenerating && "cursor-default opacity-60",
      ])}
    >
      <Sparkle
        aria-hidden
        className={cn(["size-3.5 shrink-0", isGenerating && "animate-pulse"])}
      />
      <span className={cn(["truncate", isGenerating && "animate-pulse"])}>
        {label}
      </span>
    </button>
  );
}
