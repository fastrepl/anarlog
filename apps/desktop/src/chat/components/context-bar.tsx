import { useQuery } from "@tanstack/react-query";
import { ChevronDownIcon, PlusIcon, XIcon } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn, safeParseDate } from "@hypr/utils";

import type { ContextRef } from "~/chat/context/entities";
import { type ContextChipProps, renderChip } from "~/chat/context/registry";
import type { DisplayEntity } from "~/chat/context/use-chat-context-pipeline";
import { useChatAppearance } from "~/chat/hooks/use-chat-appearance";
import { useSearchEngine } from "~/search/contexts/engine";
import { getSessionEvent } from "~/session/utils";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";

const MAX_SESSION_PICKER_RESULTS = 8;

type SessionPickerItem = {
  id: string;
  title: string;
  dateLabel: string | null;
  timestamp: number | null;
};

type MainStore = ReturnType<typeof main.UI.useStore>;

function getSessionTimestamp(row: {
  created_at?: unknown;
  event_json?: unknown;
}): number | null {
  const event =
    typeof row.event_json === "string"
      ? getSessionEvent({ event_json: row.event_json })
      : null;
  const date = safeParseDate(event?.started_at ?? row.created_at);
  return date ? date.getTime() : null;
}

function toSessionPickerItem(
  store: MainStore,
  sessionId: string,
): SessionPickerItem | null {
  if (!store?.hasRow("sessions", sessionId)) {
    return null;
  }

  const row = store.getRow("sessions", sessionId);
  const title = typeof row.title === "string" ? row.title.trim() : "";
  const timestamp = getSessionTimestamp(row);

  if (!title && timestamp === null) {
    return null;
  }

  return {
    id: sessionId,
    title: title || "Untitled",
    timestamp,
    dateLabel: timestamp ? new Date(timestamp).toLocaleDateString() : null,
  };
}

function sortByNewestSession(
  a: SessionPickerItem,
  b: SessionPickerItem,
): number {
  const aTime = a.timestamp ?? Number.NEGATIVE_INFINITY;
  const bTime = b.timestamp ?? Number.NEGATIVE_INFINITY;
  if (aTime === bTime) {
    return a.title.localeCompare(b.title);
  }
  return bTime - aTime;
}

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
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          onClick={handleClick}
          className={cn([
            "group max-w-48 min-w-0 rounded-md px-1.5 py-0.5 text-xs",
            pending
              ? "bg-muted/5 text-muted-foreground"
              : "bg-card text-muted-foreground shadow-xs",
            "inline-flex shrink items-center gap-1",
            isClickable
              ? "hover:bg-accent/20 cursor-pointer"
              : "cursor-default",
          ])}
        >
          {Icon && <Icon className="text-muted-foreground size-3 shrink-0" />}
          <span className="truncate">{chip.label}</span>
          {chip.removable && onRemove && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                onRemove(chip.key);
              }}
              className="hover:bg-accent/20 ml-0.5 hidden items-center justify-center rounded-sm group-hover:inline-flex"
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
          className="bg-muted/10 text-muted-foreground hover:bg-accent/20 hover:text-muted-foreground inline-flex shrink-0 items-center gap-0.5 rounded-md px-1 py-0.5 text-xs transition-colors"
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
  const normalizedQuery = query.trim();
  const { search } = useSearchEngine();
  const store = main.UI.useStore(main.STORE_ID);
  const sessionIds = main.UI.useRowIds("sessions", main.STORE_ID);

  const searchResults = useQuery({
    queryKey: ["chat-session-picker", normalizedQuery],
    enabled: normalizedQuery.length > 0,
    queryFn: () => search(normalizedQuery, { created_at: undefined }),
  });

  const recentSessions = useMemo(() => {
    return sessionIds
      .map((sessionId) => toSessionPickerItem(store, sessionId))
      .filter((item): item is SessionPickerItem => item !== null)
      .sort(sortByNewestSession)
      .slice(0, MAX_SESSION_PICKER_RESULTS);
  }, [sessionIds, store]);

  const matchingSessions = useMemo(() => {
    return (searchResults.data ?? [])
      .filter((hit) => hit.document.type === "session")
      .map((hit) => toSessionPickerItem(store, hit.document.id))
      .filter((item): item is SessionPickerItem => item !== null)
      .slice(0, MAX_SESSION_PICKER_RESULTS);
  }, [searchResults.data, store]);

  const results = normalizedQuery ? matchingSessions : recentSessions;

  return (
    <div className="flex flex-col gap-2">
      <input
        autoFocus
        type="text"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Search sessions..."
        className="border-border bg-card focus:border-border w-full rounded-md border px-2.5 py-1.5 text-xs outline-none"
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
            className="hover:bg-accent flex flex-col items-start rounded-md px-2 py-1.5 text-left transition-colors"
          >
            <span className="text-muted-foreground w-full truncate text-xs font-medium">
              {result.title || "Untitled"}
            </span>
            <span className="text-muted-foreground text-[10px]">
              {result.dateLabel ?? "Unknown date"}
            </span>
          </button>
        ))}
        {results.length === 0 && (
          <span className="text-muted-foreground px-2 py-1.5 text-xs">
            {searchResults.isFetching ? "Searching..." : "No sessions found"}
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
          className="bg-muted/10 text-muted-foreground hover:bg-accent/20 hover:text-muted-foreground inline-flex shrink-0 items-center justify-center rounded-md p-0.5 transition-colors"
        >
          <PlusIcon className="size-3.5" />
        </button>
      </PopoverTrigger>
      <PopoverContent variant="app" side="top" align="start" className="w-64">
        <AppFloatingPanel className="p-3">
          <SessionPicker onSelect={onAdd} onClose={() => setOpen(false)} />
        </AppFloatingPanel>
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
  const { elevatedSurfaceClassName } = useChatAppearance();
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
    <div
      data-chat-context-bar
      className={cn([
        "border-border shrink-0 rounded-t-xl border-t border-r border-l",
        elevatedSurfaceClassName,
        "mx-3",
      ])}
    >
      <div className="flex items-start gap-1.5 px-2 py-2">
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
