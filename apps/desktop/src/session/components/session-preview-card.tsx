import { useMotionValue, useSpring, useTransform } from "motion/react";
import { useCallback, useMemo, useRef, useState } from "react";
import { Streamdown } from "streamdown";

import {
  isValidTiptapContent,
  json2md,
  streamdownComponents,
} from "@hypr/tiptap/shared";
import {
  HoverCard,
  HoverCardContent,
  HoverCardTrigger,
} from "@hypr/ui/components/ui/hover-card";
import { cn, format, safeParseDate } from "@hypr/utils";

import { extractPlainText } from "~/search/contexts/engine/utils";
import {
  useEnhancedNote,
  useEnhancedNotes,
} from "~/session/hooks/useEnhancedNotes";
import * as main from "~/store/tinybase/store/main";

const MAX_PREVIEW_LENGTH = 200;
const FOLLOW_RANGE = 16;
const SPRING_CONFIG = { stiffness: 300, damping: 30, mass: 0.5 };

const OPEN_DELAY_COLD = 400;
const OPEN_DELAY_WARM = 0;
const WARMUP_COOLDOWN_MS = 600;

let lastPreviewClosedAt = 0;

function isWarmedUp() {
  return Date.now() - lastPreviewClosedAt < WARMUP_COOLDOWN_MS;
}

function markPreviewClosed() {
  lastPreviewClosedAt = Date.now();
}

function useSessionPreviewData(sessionId: string) {
  const title =
    (main.UI.useCell("sessions", sessionId, "title", main.STORE_ID) as
      | string
      | undefined) || "";
  const rawMd = main.UI.useCell(
    "sessions",
    sessionId,
    "raw_md",
    main.STORE_ID,
  ) as string | undefined;
  const createdAt = main.UI.useCell(
    "sessions",
    sessionId,
    "created_at",
    main.STORE_ID,
  ) as string | undefined;
  const eventJson = main.UI.useCell(
    "sessions",
    sessionId,
    "event_json",
    main.STORE_ID,
  ) as string | undefined;

  const participantMappingIds = main.UI.useSliceRowIds(
    main.INDEXES.sessionParticipantsBySession,
    sessionId,
    main.STORE_ID,
  );

  const enhancedNoteIds = useEnhancedNotes(sessionId);
  const firstEnhancedNoteId = enhancedNoteIds?.[0];
  const { content: enhancedContent, title: enhancedTitle } = useEnhancedNote(
    firstEnhancedNoteId ?? "",
  );

  const hasEnhanced = !!firstEnhancedNoteId && !!enhancedContent;

  const { previewMarkdown, previewPlainText } = useMemo(() => {
    const source = hasEnhanced ? (enhancedContent as string) : rawMd;
    if (typeof source !== "string" || !source.trim()) {
      return { previewMarkdown: null, previewPlainText: "" };
    }

    const trimmed = source.trim();
    if (trimmed.startsWith("{")) {
      try {
        const parsed = JSON.parse(trimmed);
        if (isValidTiptapContent(parsed)) {
          const md = json2md(parsed).trim();
          if (md) return { previewMarkdown: md, previewPlainText: "" };
        }
      } catch {}
    }

    const plain = extractPlainText(source);
    const truncated =
      plain.length > MAX_PREVIEW_LENGTH
        ? plain.slice(0, MAX_PREVIEW_LENGTH) + "…"
        : plain;
    return { previewMarkdown: null, previewPlainText: truncated };
  }, [hasEnhanced, enhancedContent, rawMd]);

  const hasContent = !!previewMarkdown || !!previewPlainText;

  const previewLabel = useMemo(() => {
    if (hasEnhanced && hasContent) {
      return (enhancedTitle as string | undefined) || "Summary";
    }
    if (hasContent) return "Notes";
    return null;
  }, [hasEnhanced, hasContent, enhancedTitle]);

  const dateDisplay = useMemo(() => {
    let timestamp = createdAt;
    if (eventJson) {
      try {
        const event = JSON.parse(eventJson);
        if (event?.started_at) timestamp = event.started_at;
      } catch {}
    }
    const parsed = safeParseDate(timestamp);
    if (!parsed) return "";
    return format(parsed, "MMM d, yyyy · h:mm a");
  }, [createdAt, eventJson]);

  return {
    title,
    previewMarkdown,
    previewPlainText,
    previewLabel,
    dateDisplay,
    participantMappingIds,
  };
}

function useCursorFollow(axis: "x" | "y") {
  const triggerRef = useRef<HTMLDivElement>(null);
  const normalized = useMotionValue(0.5);

  const offset = useSpring(
    useTransform(normalized, [0, 1], [-FOLLOW_RANGE, FOLLOW_RANGE]),
    SPRING_CONFIG,
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      const el = triggerRef.current;
      if (!el) return;
      const rect = el.getBoundingClientRect();
      const ratio =
        axis === "y"
          ? (e.clientY - rect.top) / rect.height
          : (e.clientX - rect.left) / rect.width;
      normalized.set(Math.max(0, Math.min(1, ratio)));
    },
    [axis, normalized],
  );

  const handleMouseLeave = useCallback(() => {
    normalized.set(0.5);
  }, [normalized]);

  const style = axis === "y" ? { translateY: offset } : { translateX: offset };

  return { triggerRef, handleMouseMove, handleMouseLeave, style };
}

function useParticipantNames(mappingIds: string[]) {
  const allResults = main.UI.useResultTable(
    main.QUERIES.sessionParticipantsWithDetails,
    main.STORE_ID,
  );

  return useMemo(() => {
    const names: string[] = [];
    for (const id of mappingIds) {
      const row = allResults[id];
      if (!row) continue;
      const name = (row.human_name as string) || "Unknown";
      names.push(name);
    }
    return names;
  }, [mappingIds, allResults]);
}

const MAX_VISIBLE_PARTICIPANTS = 3;

function ParticipantsList({ mappingIds }: { mappingIds: string[] }) {
  const names = useParticipantNames(mappingIds);

  if (names.length === 0) return null;

  const visible = names.slice(0, MAX_VISIBLE_PARTICIPANTS);
  const remaining = names.length - visible.length;

  return (
    <div className="line-clamp-2 text-xs text-neutral-500">
      {visible.join(", ")}
      {remaining > 0 && (
        <span className="text-neutral-500"> and {remaining} more</span>
      )}
    </div>
  );
}

export function SessionPreviewCard({
  sessionId,
  side,
  children,
  enabled = true,
}: {
  sessionId: string;
  side: "right" | "bottom";
  children: React.ReactNode;
  enabled?: boolean;
}) {
  const {
    title,
    previewMarkdown,
    previewPlainText,
    previewLabel,
    dateDisplay,
    participantMappingIds,
  } = useSessionPreviewData(sessionId);

  const followAxis = side === "right" ? "y" : "x";
  const { triggerRef, handleMouseMove, handleMouseLeave, style } =
    useCursorFollow(followAxis);

  const [openDelay, setOpenDelay] = useState(
    isWarmedUp() ? OPEN_DELAY_WARM : OPEN_DELAY_COLD,
  );

  const handleOpenChange = useCallback((open: boolean) => {
    if (open) {
      markPreviewClosed();
    } else {
      markPreviewClosed();
      setOpenDelay(OPEN_DELAY_WARM);
    }
  }, []);

  const handleMouseEnter = useCallback(() => {
    setOpenDelay(isWarmedUp() ? OPEN_DELAY_WARM : OPEN_DELAY_COLD);
  }, []);

  if (!enabled) {
    return <>{children}</>;
  }

  return (
    <HoverCard
      openDelay={openDelay}
      closeDelay={0}
      onOpenChange={handleOpenChange}
    >
      <HoverCardTrigger asChild>
        <div
          ref={triggerRef}
          onMouseMove={handleMouseMove}
          onMouseLeave={handleMouseLeave}
          onMouseEnter={handleMouseEnter}
          className={side === "bottom" ? "h-full" : ""}
        >
          {children}
        </div>
      </HoverCardTrigger>
      <HoverCardContent
        side={side}
        align="start"
        sideOffset={8}
        followStyle={style}
        className={cn(["w-72 p-4", "pointer-events-none"])}
      >
        <div className="flex flex-col gap-1">
          {dateDisplay && (
            <div className="text-xs text-neutral-500">{dateDisplay}</div>
          )}

          <div className="text-sm font-medium">{title || "Untitled"}</div>
          <ParticipantsList mappingIds={participantMappingIds} />

          {(previewMarkdown || previewPlainText) && (
            <div className="mt-1 flex flex-col gap-1">
              {previewLabel && (
                <div className="text-xs font-medium text-neutral-400">
                  {previewLabel}
                </div>
              )}
              <div className="max-h-24 overflow-hidden [mask-image:linear-gradient(to_bottom,black_60%,transparent)] text-neutral-600">
                {previewMarkdown ? (
                  <Streamdown
                    components={streamdownComponents}
                    className="flex flex-col text-xs"
                    isAnimating={false}
                  >
                    {previewMarkdown}
                  </Streamdown>
                ) : (
                  <div className="text-xs leading-relaxed">
                    {previewPlainText}
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
      </HoverCardContent>
    </HoverCard>
  );
}
