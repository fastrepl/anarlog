import { ChevronUpIcon, XCircleIcon } from "lucide-react";
import { useMemo, useState } from "react";

import { cn } from "@hypr/utils";

import { type ContextChipProps, renderChip } from "~/chat/context/registry";
import type { DisplayEntity } from "~/chat/context/use-chat-context-pipeline";
import { useTabs } from "~/store/zustand/tabs";

const COLLAPSED_CONTEXT_CHIP_LIMIT = 4;

function ContextChip({
  chip,
  onRemove,
  pending,
}: {
  chip: ContextChipProps;
  onRemove?: (key: string) => void;
  pending?: boolean;
}) {
  const Icon = chip.icon;
  const openNew = useTabs((state) => state.openNew);
  const isClickable = !!chip.entityKind && !!chip.entityId;

  const handleClick = () => {
    if (!chip.entityKind || !chip.entityId) {
      return;
    }

    if (chip.entityKind === "session") {
      openNew({ type: "sessions", id: chip.entityId });
      return;
    }

    if (chip.entityKind === "human") {
      openNew({ type: "humans", id: chip.entityId });
      return;
    }

    if (chip.entityKind === "organization") {
      openNew({ type: "organizations", id: chip.entityId });
    }
  };

  return (
    <span
      data-chat-context-chip
      onClick={handleClick}
      className={cn([
        "group border-border/60 inline-flex h-7 max-w-56 min-w-0 shrink-0 items-center gap-1.5 rounded-[10px] border px-2.5 text-xs leading-4 shadow-xs",
        pending
          ? "bg-card/60 text-muted-foreground"
          : "bg-card/90 text-muted-foreground",
        isClickable ? "hover:bg-accent/20 cursor-pointer" : "cursor-default",
      ])}
    >
      <span className="relative flex size-4 shrink-0 items-center justify-center">
        <Icon
          className={cn([
            "text-muted-foreground size-3.5 shrink-0 transition-opacity",
            chip.removable && onRemove ? "group-hover:opacity-0" : "",
          ])}
        />
        {chip.removable && onRemove && (
          <button
            type="button"
            aria-label={`Remove ${chip.label}`}
            onClick={(e) => {
              e.stopPropagation();
              onRemove(chip.key);
            }}
            className="text-muted-foreground hover:text-foreground pointer-events-none absolute inset-0 flex items-center justify-center opacity-0 transition-opacity group-hover:pointer-events-auto group-hover:opacity-100 focus-visible:pointer-events-auto focus-visible:opacity-100"
          >
            <XCircleIcon className="size-3.5" />
          </button>
        )}
      </span>
      <span className="truncate">{chip.label}</span>
    </span>
  );
}

function ChipList({
  chips,
  onRemove,
}: {
  chips: Array<{ chip: ContextChipProps; pending: boolean }>;
  onRemove?: (key: string) => void;
}) {
  const [isExpanded, setIsExpanded] = useState(false);
  const hiddenCount = Math.max(0, chips.length - COLLAPSED_CONTEXT_CHIP_LIMIT);
  const visibleChips = isExpanded
    ? chips
    : chips.slice(0, COLLAPSED_CONTEXT_CHIP_LIMIT);
  const canExpand = hiddenCount > 0;

  return (
    <div className="flex w-full min-w-0 items-center justify-center gap-1.5">
      <div
        data-chat-context-chip-list
        className={cn([
          "flex max-w-full min-w-0 items-center justify-center gap-1.5",
          canExpand && !isExpanded ? "overflow-hidden" : "flex-wrap",
        ])}
      >
        {visibleChips.map(({ chip, pending }) => (
          <ContextChip
            key={chip.key}
            chip={chip}
            onRemove={onRemove}
            pending={pending}
          />
        ))}
      </div>

      {canExpand && (
        <button
          type="button"
          data-chat-context-overflow-chip
          aria-expanded={isExpanded}
          aria-label={isExpanded ? "Collapse context chips" : undefined}
          onClick={() => setIsExpanded((value) => !value)}
          className="bg-card/70 border-border/60 text-muted-foreground hover:bg-accent/20 hover:text-muted-foreground inline-flex h-7 shrink-0 items-center gap-0.5 rounded-[10px] border px-1.5 text-xs shadow-xs transition-colors"
        >
          {isExpanded ? (
            <ChevronUpIcon aria-hidden="true" className="size-3" />
          ) : (
            `+${hiddenCount} more`
          )}
        </button>
      )}
    </div>
  );
}

export function ContextBar({
  entities,
  onRemoveEntity,
}: {
  entities: DisplayEntity[];
  onRemoveEntity?: (key: string) => void;
}) {
  const chips = useMemo(
    () =>
      entities
        .map((entity) => ({
          chip: renderChip(entity),
          pending: entity.pending,
        }))
        .filter(
          (c): c is { chip: ContextChipProps; pending: boolean } =>
            c.chip !== null,
        ),
    [entities],
  );

  if (chips.length === 0) {
    return null;
  }

  return (
    <div data-chat-context-bar className={cn(["shrink-0 px-3 pb-1.5"])}>
      <ChipList chips={chips} onRemove={onRemoveEntity} />
    </div>
  );
}
