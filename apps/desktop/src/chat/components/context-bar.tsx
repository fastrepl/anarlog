import { ChevronDownIcon, PlusIcon, XIcon } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import type { ContextRef } from "~/chat/context/entities";
import { type ContextChipProps, renderChip } from "~/chat/context/registry";
import type { DisplayEntity } from "~/chat/context/use-chat-context-pipeline";
import { useSearchEngine } from "~/search/contexts/engine";
import { useTabs } from "~/store/zustand/tabs";

function useOverflow(
  ref: React.RefObject<HTMLDivElement | null>,
  deps: unknown[],
) {
  const [hasOverflow, setHasOverflow] = useState(false);
  const [hiddenCount, setHiddenCount] = useState(0);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const check = () => {
      const overflows = el.scrollHeight > el.clientHeight;
      setHasOverflow(overflows);

      if (overflows) {
        const cutoff = el.getBoundingClientRect().bottom;
        let hidden = 0;
        for (const child of el.children) {
          if ((child as HTMLElement).getBoundingClientRect().top >= cutoff) {
            hidden++;
          }
        }
        setHiddenCount(hidden);
      } else {
        setHiddenCount(0);
      }
    };

    const observer = new ResizeObserver(check);
    observer.observe(el);
    check();
    return () => observer.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);

  return { hasOverflow, hiddenCount };
}

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
  const isClickable = chip.entityKind === "session" && chip.entityId;

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          onClick={() => {
            if (isClickable) {
              openNew({ type: "sessions", id: chip.entityId! });
            }
          }}
          className={cn([
            "group max-w-48 min-w-0 rounded-md px-1.5 py-0.5 text-xs",
            pending
              ? "bg-neutral-500/5 text-neutral-400"
              : "bg-white text-neutral-600 shadow-xs",
            "inline-flex shrink items-center gap-1",
            isClickable
              ? "cursor-pointer hover:bg-neutral-500/20"
              : "cursor-default",
          ])}
        >
          {Icon && <Icon className="size-3 shrink-0 text-neutral-400" />}
          <span className="truncate">{chip.label}</span>
          {chip.removable && onRemove && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                onRemove(chip.key);
              }}
              className="ml-0.5 hidden items-center justify-center rounded-sm group-hover:inline-flex hover:bg-neutral-500/20"
            >
              <XIcon className="size-2.5" />
            </button>
          )}
        </span>
      </TooltipTrigger>
      <TooltipContent side="top" className="z-110">
        {chip.tooltip}
      </TooltipContent>
    </Tooltip>
  );
}

function ChipList({
  chips,
  onRemove,
}: {
  chips: Array<{ chip: ContextChipProps; pending: boolean }>;
  onRemove?: (key: string) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [expanded, setExpanded] = useState(false);
  const { hasOverflow, hiddenCount } = useOverflow(ref, [chips]);

  useEffect(() => {
    setExpanded(false);
  }, [chips.length]);

  const showToggle = hasOverflow || expanded;

  return (
    <div className="flex items-start gap-1.5">
      <div
        ref={ref}
        className={cn([
          "flex min-w-0 flex-1 flex-wrap items-center gap-1.5",
          !expanded && "max-h-[22px] overflow-hidden",
        ])}
      >
        {chips.map(({ chip, pending }) => (
          <ContextChip
            key={chip.key}
            chip={chip}
            onRemove={onRemove}
            pending={pending}
          />
        ))}
      </div>

      {showToggle && (
        <button
          type="button"
          onClick={() => setExpanded((v) => !v)}
          className="inline-flex shrink-0 items-center gap-0.5 rounded-md bg-neutral-500/10 px-1 py-0.5 text-xs text-neutral-400 transition-colors hover:bg-neutral-500/20 hover:text-neutral-600"
        >
          {!expanded && hiddenCount > 0 && <span>+{hiddenCount}</span>}
          <ChevronDownIcon
            className={cn(["size-3.5", expanded && "rotate-180"])}
          />
        </button>
      )}
    </div>
  );
}

function SessionPicker({
  onSelect,
  onClose,
}: {
  onSelect: (sessionId: string) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<
    Array<{ id: string; title: string; created_at: number }>
  >([]);
  const { search } = useSearchEngine();

  useEffect(() => {
    search(query, { created_at: undefined }).then((hits) => {
      setResults(
        hits
          .filter((h) => h.document.type === "session")
          .slice(0, 8)
          .map((h) => ({
            id: h.document.id,
            title: h.document.title,
            created_at: h.document.created_at,
          })),
      );
    });
  }, [query, search]);

  return (
    <div className="flex flex-col gap-2">
      <input
        autoFocus
        type="text"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Search sessions..."
        className="w-full rounded-md border border-neutral-200 bg-white px-2.5 py-1.5 text-xs outline-none focus:border-neutral-400"
      />
      <div className="flex max-h-48 flex-col gap-0.5 overflow-y-auto">
        {results.map((result) => (
          <button
            key={result.id}
            type="button"
            onClick={() => {
              onSelect(result.id);
              onClose();
            }}
            className="flex flex-col items-start rounded-md px-2 py-1.5 text-left transition-colors hover:bg-neutral-100"
          >
            <span className="w-full truncate text-xs font-medium text-neutral-700">
              {result.title || "Untitled"}
            </span>
            <span className="text-[10px] text-neutral-400">
              {new Date(result.created_at).toLocaleDateString()}
            </span>
          </button>
        ))}
        {results.length === 0 && (
          <span className="px-2 py-1.5 text-xs text-neutral-400">
            No sessions found
          </span>
        )}
      </div>
    </div>
  );
}

function AddSessionButton({ onAdd }: { onAdd: (sessionId: string) => void }) {
  const [open, setOpen] = useState(false);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="inline-flex shrink-0 items-center justify-center rounded-md bg-neutral-500/10 p-0.5 text-neutral-400 transition-colors hover:bg-neutral-500/20 hover:text-neutral-600"
        >
          <PlusIcon className="size-3.5" />
        </button>
      </PopoverTrigger>
      <PopoverContent side="top" align="start" className="w-64 p-3">
        <SessionPicker onSelect={onAdd} onClose={() => setOpen(false)} />
      </PopoverContent>
    </Popover>
  );
}

export function ContextBar({
  entities,
  onRemoveEntity,
  onAddEntity,
}: {
  entities: DisplayEntity[];
  onRemoveEntity?: (key: string) => void;
  onAddEntity?: (ref: ContextRef) => void;
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

  if (chips.length === 0 && !onAddEntity) {
    return null;
  }

  return (
    <div className="mx-2 shrink-0 rounded-t-xl border-t border-r border-l border-neutral-200 bg-neutral-100">
      <div className="flex items-start gap-1.5 px-2.5 py-2">
        <div className="min-w-0 flex-1">
          <ChipList chips={chips} onRemove={onRemoveEntity} />
        </div>

        {onAddEntity && (
          <AddSessionButton
            onAdd={(sessionId) => {
              onAddEntity({
                kind: "session",
                key: `session:manual:${sessionId}`,
                source: "manual",
                sessionId,
              });
            }}
          />
        )}
      </div>
    </div>
  );
}
