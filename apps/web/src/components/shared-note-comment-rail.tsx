import { LoaderCircleIcon, Trash2Icon, UserRoundIcon } from "lucide-react";
import { useRef, useState } from "react";

import { cn } from "@hypr/utils";

import type { AnchoredSharedNoteComment } from "@/lib/shared-note-comment-anchors";
import { layoutRailCards } from "@/lib/shared-note-comment-rail-layout";

export const DRAFT_COMMENT_ID = "draft";

const RAIL_CARD_GAP = 10;

export function SharedNoteCommentRail({
  activeCommentId,
  canDelete,
  composer,
  composerNode,
  deletePending = false,
  deletingCommentId = null,
  items,
  onActivate,
  onDelete,
  screenTops,
}: {
  items: AnchoredSharedNoteComment[];
  screenTops: ReadonlyMap<string, number>;
  activeCommentId: string | null;
  onActivate: (commentId: string | null) => void;
  composer: { top: number } | null;
  composerNode?: React.ReactNode;
  onDelete: (commentId: string) => void;
  canDelete: (comment: AnchoredSharedNoteComment) => boolean;
  deletePending?: boolean;
  deletingCommentId?: string | null;
}) {
  const [heights, setHeights] = useState<ReadonlyMap<string, number>>(
    new Map(),
  );
  // Cached per card so React only re-runs a ref (and its ResizeObserver
  // cleanup) when the card unmounts, not on every render.
  const measureRefs = useRef(
    new Map<string, (element: HTMLDivElement | null) => (() => void) | void>(),
  );

  const measureRef = (id: string) => {
    const cached = measureRefs.current.get(id);
    if (cached) return cached;
    const ref = (element: HTMLDivElement | null) => {
      if (!element) return;
      const observer = new ResizeObserver(() => {
        const height = element.getBoundingClientRect().height;
        setHeights((previous) =>
          previous.get(id) === height
            ? previous
            : new Map(previous).set(id, height),
        );
      });
      observer.observe(element);
      return () => {
        observer.disconnect();
        setHeights((previous) => {
          if (!previous.has(id)) return previous;
          const next = new Map(previous);
          next.delete(id);
          return next;
        });
      };
    };
    measureRefs.current.set(id, ref);
    return ref;
  };

  const anchoredItems = items.filter((item) => item.range !== null);
  const placements = layoutRailCards(
    [
      ...(composer
        ? [
            {
              id: DRAFT_COMMENT_ID,
              desiredTop: composer.top,
              height: heights.get(DRAFT_COMMENT_ID) ?? 0,
            },
          ]
        : []),
      ...anchoredItems.map((item) => ({
        id: item.commentId,
        desiredTop: screenTops.get(item.commentId) ?? 0,
        height: heights.get(item.commentId) ?? 0,
      })),
    ],
    {
      activeId: composer ? DRAFT_COMMENT_ID : activeCommentId,
      gap: RAIL_CARD_GAP,
    },
  );
  const topById = new Map(
    placements.map((placement) => [placement.id, placement.top]),
  );

  if (!composer && anchoredItems.length === 0) {
    return null;
  }

  return (
    <div className="relative h-full">
      {composer && (
        <div
          ref={measureRef(DRAFT_COMMENT_ID)}
          className="absolute inset-x-0 transition-[top] duration-200"
          style={{ top: topById.get(DRAFT_COMMENT_ID) ?? composer.top }}
        >
          {composerNode}
        </div>
      )}
      {anchoredItems.map((item) => (
        <div
          key={item.commentId}
          ref={measureRef(item.commentId)}
          className="absolute inset-x-0 transition-[top] duration-200"
          style={{ top: topById.get(item.commentId) ?? 0 }}
        >
          <SharedNoteCommentCard
            active={item.commentId === activeCommentId}
            comment={item}
            deleteDisabled={deletePending}
            deleting={deletePending && item.commentId === deletingCommentId}
            onActivate={() =>
              onActivate(
                item.commentId === activeCommentId ? null : item.commentId,
              )
            }
            onDelete={() => onDelete(item.commentId)}
            showDelete={canDelete(item)}
          />
        </div>
      ))}
    </div>
  );
}

export function SharedNoteCommentCard({
  active,
  comment,
  deleteDisabled = false,
  deleting = false,
  onActivate,
  onDelete,
  showDelete,
}: {
  active: boolean;
  comment: AnchoredSharedNoteComment;
  deleteDisabled?: boolean;
  deleting?: boolean;
  onActivate?: () => void;
  onDelete: () => void;
  showDelete: boolean;
}) {
  return (
    <div
      role={onActivate ? "button" : undefined}
      tabIndex={onActivate ? 0 : undefined}
      onClick={onActivate}
      onKeyDown={
        onActivate
          ? (event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onActivate();
              }
            }
          : undefined
      }
      className={cn([
        "surface rounded-xl border p-3.5 text-left shadow-sm transition-[border-color,box-shadow]",
        active ? "border-stone-400 shadow-md" : "border-color-subtle",
        onActivate && "cursor-pointer",
      ])}
    >
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          <span className="surface-subtle text-color-muted flex size-6 shrink-0 items-center justify-center rounded-full">
            <UserRoundIcon className="size-3.5" aria-hidden="true" />
          </span>
          <p className="text-color min-w-0 truncate text-xs font-medium">
            {comment.isAuthor ? "You" : "Collaborator"}{" "}
            <time
              className="text-color-muted font-normal"
              dateTime={comment.createdAt}
            >
              {formatRelativeTime(comment.createdAt)}
            </time>
          </p>
        </div>
        {showDelete && (
          <button
            type="button"
            aria-label="Delete comment"
            className="text-color-muted hover:text-color rounded-full p-1.5 transition-colors focus-visible:ring-2 focus-visible:ring-stone-500 focus-visible:outline-hidden disabled:cursor-not-allowed disabled:opacity-50"
            disabled={deleteDisabled}
            onClick={(event) => {
              event.stopPropagation();
              onDelete();
            }}
          >
            {deleting ? (
              <LoaderCircleIcon
                className="size-3.5 animate-spin"
                aria-hidden="true"
              />
            ) : (
              <Trash2Icon className="size-3.5" aria-hidden="true" />
            )}
          </button>
        )}
      </div>
      <p className="text-color mt-3 text-sm leading-5 whitespace-pre-wrap">
        {comment.body}
      </p>
    </div>
  );
}

const RELATIVE_TIME_DIVISIONS: Array<
  [amount: number, unit: Intl.RelativeTimeFormatUnit]
> = [
  [60, "second"],
  [60, "minute"],
  [24, "hour"],
  [7, "day"],
  [4.34524, "week"],
  [12, "month"],
  [Number.POSITIVE_INFINITY, "year"],
];

function formatRelativeTime(value: string) {
  const formatter = new Intl.RelativeTimeFormat("en-US", { numeric: "auto" });
  let duration = (Date.parse(value) - Date.now()) / 1000;
  for (const [amount, unit] of RELATIVE_TIME_DIVISIONS) {
    if (Math.abs(duration) < amount) {
      return formatter.format(Math.round(duration), unit);
    }
    duration /= amount;
  }
  return "";
}
