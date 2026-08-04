import { ArrowUp, CircleNotch, DotsThree } from "@phosphor-icons/react";
import { useRef, useState } from "react";

import { Avatar } from "@anlg/ui/components/avatar";
import { cn } from "@anlg/utils";

import {
  MAX_SHARED_NOTE_COMMENT_BYTES,
  validateSharedNoteCommentBody,
} from "@/lib/shared-note-collaboration";
import type { AnchoredSharedNoteComment } from "@/lib/shared-note-comment-anchors";
import { layoutRailCards } from "@/lib/shared-note-comment-rail-layout";
import { groupSharedNoteCommentThreads } from "@/lib/shared-note-comment-threads";
import { formatSharedNoteRelativeTime } from "@/lib/shared-note-presentation";

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
  onReply,
  screenTops,
}: {
  items: AnchoredSharedNoteComment[];
  screenTops: ReadonlyMap<string, number>;
  activeCommentId: string | null;
  onActivate: (commentId: string | null) => void;
  composer: { top: number } | null;
  composerNode?: React.ReactNode;
  onDelete: (commentId: string) => void;
  onReply?: (
    comment: AnchoredSharedNoteComment,
    body: string,
  ) => Promise<unknown>;
  canDelete: (comment: AnchoredSharedNoteComment) => boolean;
  deletePending?: boolean;
  deletingCommentId?: string | null;
}) {
  const [heights, setHeights] = useState<ReadonlyMap<string, number>>(
    new Map(),
  );
  const measureRefs = useRef(
    new Map<string, (element: HTMLDivElement | null) => (() => void) | void>(),
  );

  const measureRef = (id: string) => {
    const cached = measureRefs.current.get(id);
    if (cached) return cached;
    const ref = (element: HTMLDivElement | null) => {
      if (!element) {
        if (measureRefs.current.get(id) === ref) {
          measureRefs.current.delete(id);
        }
        return;
      }
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
        if (measureRefs.current.get(id) === ref) {
          measureRefs.current.delete(id);
        }
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

  const threads = groupSharedNoteCommentThreads(
    items.filter((item) => item.range !== null),
  );
  const activeThread =
    threads.find((thread) =>
      thread.comments.some((comment) => comment.commentId === activeCommentId),
    ) ?? null;
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
      ...threads.map((thread) => ({
        id: thread.id,
        desiredTop: screenTops.get(thread.comments[0].commentId) ?? 0,
        height: heights.get(thread.id) ?? 0,
      })),
    ],
    {
      activeId: composer
        ? DRAFT_COMMENT_ID
        : (activeThread?.id ?? activeCommentId),
      gap: RAIL_CARD_GAP,
    },
  );
  const topById = new Map(
    placements.map((placement) => [placement.id, placement.top]),
  );

  if (!composer && threads.length === 0) {
    return null;
  }

  return (
    <div className="relative h-full">
      {composer ? (
        <div
          ref={measureRef(DRAFT_COMMENT_ID)}
          className="absolute inset-x-0 transition-[top] duration-200"
          style={{ top: topById.get(DRAFT_COMMENT_ID) ?? composer.top }}
        >
          {composerNode}
        </div>
      ) : null}
      {threads.map((thread) => {
        const active = thread.id === activeThread?.id;
        return (
          <div
            key={thread.id}
            ref={measureRef(thread.id)}
            className="absolute inset-x-0 transition-[top] duration-200"
            style={{ top: topById.get(thread.id) ?? 0 }}
          >
            <SharedNoteCommentCard
              active={active}
              canDelete={canDelete}
              comment={thread.comments[0]}
              deleteDisabled={deletePending}
              deletingCommentId={deletingCommentId}
              onActivate={() => onActivate(active ? null : thread.id)}
              onDelete={onDelete}
              onReply={onReply}
              replies={thread.comments.slice(1)}
            />
          </div>
        );
      })}
    </div>
  );
}

export function SharedNoteCommentCard({
  active,
  canDelete,
  comment,
  deleteDisabled = false,
  deletingCommentId = null,
  onActivate,
  onDelete,
  onReply,
  replies = [],
}: {
  active: boolean;
  canDelete: (comment: AnchoredSharedNoteComment) => boolean;
  comment: AnchoredSharedNoteComment;
  deleteDisabled?: boolean;
  deletingCommentId?: string | null;
  onActivate?: () => void;
  onDelete: (commentId: string) => void;
  onReply?: (
    comment: AnchoredSharedNoteComment,
    body: string,
  ) => Promise<unknown>;
  replies?: AnchoredSharedNoteComment[];
}) {
  return (
    <div
      role={onActivate ? "button" : undefined}
      tabIndex={onActivate ? 0 : undefined}
      onClick={onActivate}
      onKeyDown={
        onActivate
          ? (event) => {
              if (event.target !== event.currentTarget) return;
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onActivate();
              }
            }
          : undefined
      }
      className={cn([
        "overflow-hidden rounded-[22px] border bg-white text-left shadow-[0_7px_22px_rgba(0,0,0,0.08)]",
        "transition-[border-color,box-shadow]",
        active ? "border-stone-400 shadow-lg" : "border-stone-300",
        onActivate && "cursor-pointer",
      ])}
    >
      <div className="p-3.5">
        <CommentHeader
          canDelete={canDelete(comment)}
          comment={comment}
          deleteDisabled={deleteDisabled}
          deleting={deletingCommentId === comment.commentId}
          onDelete={onDelete}
        />
        <p className="mt-3 text-sm leading-[1.5] whitespace-pre-wrap text-stone-800">
          {comment.body}
        </p>
      </div>
      {replies.length ? (
        <div
          className="border-t border-stone-200"
          onClick={(event) => event.stopPropagation()}
        >
          {replies.map((reply) => (
            <div
              key={reply.commentId}
              className="flex gap-2.5 border-b border-stone-100 px-3.5 py-3 last:border-b-0"
            >
              <Avatar
                seed={
                  reply.isAuthor
                    ? "shared-note:you"
                    : "shared-note:collaborator"
                }
                label={reply.isAuthor ? "You" : "Collaborator"}
                size={24}
              />
              <div className="min-w-0 flex-1">
                <div className="flex items-start justify-between gap-2">
                  <p className="min-w-0 truncate text-[11px] font-semibold text-stone-700">
                    {reply.isAuthor ? "You" : "Collaborator"}{" "}
                    <time
                      className="font-normal text-stone-500"
                      dateTime={reply.createdAt}
                    >
                      {formatSharedNoteRelativeTime(reply.createdAt)}
                    </time>
                  </p>
                  <CommentActions
                    canDelete={canDelete(reply)}
                    commentId={reply.commentId}
                    deleteDisabled={deleteDisabled}
                    deleting={deletingCommentId === reply.commentId}
                    onDelete={onDelete}
                  />
                </div>
                <p className="mt-1 text-xs leading-[1.45] whitespace-pre-wrap text-stone-700">
                  {reply.body}
                </p>
              </div>
            </div>
          ))}
        </div>
      ) : null}
      {active && onReply && comment.anchor ? (
        <ReplyComposer comment={comment} onReply={onReply} />
      ) : null}
    </div>
  );
}

function CommentHeader({
  canDelete,
  comment,
  deleteDisabled,
  deleting,
  onDelete,
}: {
  canDelete: boolean;
  comment: AnchoredSharedNoteComment;
  deleteDisabled: boolean;
  deleting: boolean;
  onDelete: (commentId: string) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-3">
      <div className="flex min-w-0 items-center gap-2">
        <Avatar
          seed={
            comment.isAuthor ? "shared-note:you" : "shared-note:collaborator"
          }
          label={comment.isAuthor ? "You" : "Collaborator"}
          size={25}
        />
        <p className="min-w-0 truncate text-xs font-semibold text-stone-800">
          {comment.isAuthor ? "You" : "Collaborator"}{" "}
          <time
            className="font-normal text-stone-500"
            dateTime={comment.createdAt}
          >
            {formatSharedNoteRelativeTime(comment.createdAt)}
          </time>
        </p>
      </div>
      <CommentActions
        canDelete={canDelete}
        commentId={comment.commentId}
        deleteDisabled={deleteDisabled}
        deleting={deleting}
        onDelete={onDelete}
      />
    </div>
  );
}

function CommentActions({
  canDelete,
  commentId,
  deleteDisabled,
  deleting,
  onDelete,
}: {
  canDelete: boolean;
  commentId: string;
  deleteDisabled: boolean;
  deleting: boolean;
  onDelete: (commentId: string) => void;
}) {
  const [open, setOpen] = useState(false);
  if (!canDelete) return null;

  return (
    <div className="relative shrink-0">
      <button
        type="button"
        aria-label="Comment actions"
        aria-expanded={open}
        className="grid size-6 place-items-center rounded-md text-stone-500 hover:bg-stone-100 hover:text-stone-700 focus-visible:ring-2 focus-visible:ring-stone-900 focus-visible:outline-hidden"
        disabled={deleteDisabled}
        onClick={(event) => {
          event.stopPropagation();
          setOpen((current) => !current);
        }}
      >
        {deleting ? (
          <CircleNotch className="size-3.5 animate-spin" aria-hidden="true" />
        ) : (
          <DotsThree className="size-4" aria-hidden="true" />
        )}
      </button>
      {open ? (
        <div
          className="absolute top-7 right-0 z-20 w-28 rounded-lg border border-stone-200 bg-white p-1 shadow-lg"
          onClick={(event) => event.stopPropagation()}
        >
          <button
            type="button"
            className="w-full rounded-md px-2 py-1.5 text-left text-xs text-stone-700 hover:bg-stone-100"
            disabled={deleteDisabled}
            onClick={() => {
              setOpen(false);
              onDelete(commentId);
            }}
          >
            Delete
          </button>
        </div>
      ) : null}
    </div>
  );
}

function ReplyComposer({
  comment,
  onReply,
}: {
  comment: AnchoredSharedNoteComment;
  onReply: (
    comment: AnchoredSharedNoteComment,
    body: string,
  ) => Promise<unknown>;
}) {
  const [body, setBody] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState(false);
  const validated = validateSharedNoteCommentBody(body);
  const tooLong = validated.byteLength > MAX_SHARED_NOTE_COMMENT_BYTES;

  const submit = async () => {
    if (!validated.valid || pending) return;
    setPending(true);
    setError(false);
    try {
      await onReply(comment, validated.body);
      setBody("");
    } catch {
      setError(true);
    } finally {
      setPending(false);
    }
  };

  return (
    <form
      className="border-t border-stone-200 px-3 py-2.5"
      onClick={(event) => event.stopPropagation()}
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <div className="flex items-end gap-2 rounded-[10px] border border-stone-300 bg-white p-1.5 pl-2 focus-within:border-stone-400 focus-within:ring-2 focus-within:ring-stone-200">
        <Avatar seed="shared-note:you" label="You" size={24} />
        <textarea
          aria-label="Reply to comment"
          aria-invalid={tooLong}
          className="max-h-20 min-h-6 min-w-0 flex-1 resize-none bg-transparent py-0.5 text-xs leading-[18px] text-stone-800 placeholder:text-stone-400 focus:outline-hidden"
          placeholder="Reply…"
          rows={1}
          value={body}
          onChange={(event) => setBody(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              void submit();
            }
          }}
        />
        <button
          type="submit"
          aria-label="Send reply"
          className="grid size-6 shrink-0 place-items-center rounded-full bg-stone-900 text-white disabled:bg-stone-200 disabled:text-stone-400"
          disabled={!validated.valid || pending}
        >
          {pending ? (
            <CircleNotch className="size-3.5 animate-spin" aria-hidden="true" />
          ) : (
            <ArrowUp className="size-3.5" aria-hidden="true" />
          )}
        </button>
      </div>
      {tooLong ? (
        <p className="mt-1.5 text-[11px] text-red-700" role="alert">
          Reply is too long ({validated.byteLength.toLocaleString()}/
          {MAX_SHARED_NOTE_COMMENT_BYTES.toLocaleString()} bytes).
        </p>
      ) : null}
      {error ? (
        <p className="mt-1.5 text-[11px] text-red-700" role="status">
          Your reply couldn’t be added. Try again.
        </p>
      ) : null}
    </form>
  );
}
