import { t } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import {
  ArrowsMerge,
  CheckCircle,
  Selection,
  UserSwitch,
} from "@phosphor-icons/react";
import { useCallback, useState } from "react";

import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";
import { cn } from "@anlg/utils";

import type { TranscriptWordSelection } from "./selection";
import { SpeakerParticipantPicker } from "./speaker-assign";

const TOOLBAR_BUTTON_CLASSES = [
  "flex h-7 items-center gap-1.5 rounded-full px-2 text-sm font-medium",
  "hover:bg-accent focus-visible:ring-ring transition-colors focus-visible:ring-2 focus-visible:outline-hidden",
];

export function TranscriptSelectButton({
  selectMode,
  onSelectModeChange,
}: {
  selectMode: boolean;
  onSelectModeChange: (selectMode: boolean) => void;
}) {
  return (
    <button
      type="button"
      data-tauri-drag-region="false"
      aria-pressed={selectMode}
      onClick={() => onSelectModeChange(!selectMode)}
      className={cn([
        "border-border bg-card text-foreground flex h-7 items-center gap-1.5 rounded-full border px-2 text-sm font-medium",
        "hover:bg-accent focus-visible:ring-ring transition-colors focus-visible:ring-2 focus-visible:outline-hidden",
        selectMode ? "border-primary/30 bg-primary/10 text-primary" : null,
      ])}
    >
      {selectMode ? (
        <CheckCircle aria-hidden className="size-3.5" />
      ) : (
        <Selection aria-hidden className="size-3.5" />
      )}
      {selectMode ? <Trans>Done</Trans> : <Trans>Select</Trans>}
    </button>
  );
}

export function TranscriptSelectToolbar({
  selection,
  entryCount,
  canMerge,
  onSelectAll,
  onClear,
  onDone,
  onAssignSpeaker,
  onMerge,
}: {
  selection: TranscriptWordSelection | null;
  entryCount: number;
  canMerge: boolean;
  onSelectAll: () => void;
  onClear: () => void;
  onDone: () => void;
  onAssignSpeaker: (
    selection: TranscriptWordSelection,
    humanId: string,
  ) => void | Promise<void>;
  onMerge: () => void | Promise<void>;
}) {
  const [speakerPickerOpen, setSpeakerPickerOpen] = useState(false);
  const handleAssign = useCallback(
    async (humanId: string) => {
      if (!selection) {
        return;
      }
      await onAssignSpeaker(selection, humanId);
      setSpeakerPickerOpen(false);
      onClear();
    },
    [onAssignSpeaker, onClear, selection],
  );
  const handleMerge = useCallback(async () => {
    await onMerge();
    onClear();
  }, [onClear, onMerge]);

  return (
    <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
      <span className="text-muted-foreground px-1 text-xs whitespace-nowrap">
        <Trans>{entryCount} selected</Trans>
      </span>
      <button
        type="button"
        className={cn(TOOLBAR_BUTTON_CLASSES)}
        onClick={onSelectAll}
      >
        <Trans>Select All</Trans>
      </button>
      <button
        type="button"
        className={cn(TOOLBAR_BUTTON_CLASSES)}
        disabled={entryCount === 0}
        onClick={onClear}
      >
        <Trans>Clear</Trans>
      </button>
      <Popover open={speakerPickerOpen} onOpenChange={setSpeakerPickerOpen}>
        <PopoverTrigger asChild>
          <button
            type="button"
            disabled={!selection}
            className={cn([
              "bg-primary text-primary-foreground hover:bg-primary/90 flex h-7 items-center gap-1.5 rounded-full px-3 text-sm font-medium",
              "disabled:pointer-events-none disabled:opacity-50",
            ])}
          >
            <UserSwitch className="size-3.5" />
            <Trans>Change speaker</Trans>
          </button>
        </PopoverTrigger>
        {selection && (
          <PopoverContent
            variant="app"
            side="bottom"
            align="end"
            sideOffset={8}
            className="w-80"
          >
            <SpeakerParticipantPicker
              sessionId={selection.sessionId}
              showAssignmentScope={false}
              onSelect={handleAssign}
            />
          </PopoverContent>
        )}
      </Popover>
      <button
        type="button"
        disabled={!canMerge}
        className={cn([
          TOOLBAR_BUTTON_CLASSES,
          "disabled:pointer-events-none disabled:opacity-50",
        ])}
        onClick={() => void handleMerge()}
      >
        <ArrowsMerge className="size-3.5" />
        <Trans>Merge</Trans>
      </button>
      <button
        type="button"
        aria-label={t`Done`}
        className={cn([TOOLBAR_BUTTON_CLASSES, "border-border ml-auto border"])}
        onClick={onDone}
      >
        <CheckCircle className="size-3.5" />
        <Trans>Done</Trans>
      </button>
    </div>
  );
}
