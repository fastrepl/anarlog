import { ArrowUp, CircleNotch, DotsThree } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
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
const styles = stylex.create({
  style1: {
    position: "relative",
    height: "100%",
  },
  style2: {
    position: "absolute",
    insetInline: "0",
    transitionProperty: "top",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".2s",
    "--tw-duration": ".2s",
  },
  style3: {
    padding: ".875rem",
  },
  style4: {
    marginTop: ".75rem",
    fontSize: ".875rem",
    lineHeight: "1.5",
    "--tw-leading": "1.5",
    whiteSpace: "pre-wrap",
    color: "#292524",
  },
  style5: {
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    borderColor: "#e7e5e4",
  },
  style6: {
    display: "flex",
    gap: ".625rem",
    borderBottomStyle: {
      default: "solid",
      ":last-child": "solid",
    },
    borderBottomWidth: {
      default: "1px",
      ":last-child": "0",
    },
    borderColor: "#f5f5f4",
    paddingInline: ".875rem",
    paddingBlock: ".75rem",
  },
  style7: {
    minWidth: "0",
    flex: "1",
  },
  style8: {
    display: "flex",
    alignItems: "flex-start",
    justifyContent: "space-between",
    gap: ".5rem",
  },
  style9: {
    minWidth: "0",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    overflow: "hidden",
    fontSize: "11px",
    "--tw-font-weight": "600",
    fontWeight: "600",
    color: "#44403c",
  },
  style10: {
    "--tw-font-weight": "400",
    fontWeight: "400",
    color: "#78716c",
  },
  style11: {
    marginTop: ".25rem",
    fontSize: ".75rem",
    lineHeight: "1.45",
    "--tw-leading": "1.45",
    whiteSpace: "pre-wrap",
    color: "#44403c",
  },
  style12: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: ".75rem",
  },
  style13: {
    display: "flex",
    minWidth: "0",
    alignItems: "center",
    gap: ".5rem",
  },
  style14: {
    minWidth: "0",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    overflow: "hidden",
    fontSize: ".75rem",
    lineHeight: "1rem",
    "--tw-font-weight": "600",
    fontWeight: "600",
    color: "#292524",
  },
  style15: {
    position: "relative",
    flexShrink: "0",
  },
  style16: {
    display: "grid",
    width: "1.5rem",
    height: "1.5rem",
    placeItems: "center",
    borderRadius: ".375rem",
    color: {
      default: "#78716c",
      ":hover": "#44403c",
    },
    backgroundColor: {
      default: null,
      ":hover": "#f5f5f4",
    },
    "--tw-ring-shadow": {
      default: null,
      ":focus-visible": " 0 0 0 calc(2px + 0) currentcolor",
    },
    boxShadow: {
      default: null,
      ":focus-visible":
        "0 0 #0000, 0 0 #0000, 0 0 #0000, var(--tw-ring-inset, ) 0 0 0 calc(2px + var(--tw-ring-offset-width)) var(--tw-ring-color, currentcolor), 0 0 #0000",
    },
    "--tw-ring-color": {
      default: null,
      ":focus-visible": "#1c1917",
    },
    "--tw-outline-style": {
      default: null,
      ":focus-visible": "none",
    },
    outlineStyle: {
      default: null,
      ":focus-visible": "none",
    },
    outlineOffset: {
      default: null,
      "@media (forced-colors: active)": {
        default: null,
        ":focus-visible": "2px",
      },
    },
    outline: {
      default: null,
      "@media (forced-colors: active)": {
        default: null,
        ":focus-visible": "2px solid #0000",
      },
    },
  },
  style17: {
    width: ".875rem",
    height: ".875rem",
    animation: "1s linear infinite spin",
  },
  style18: {
    width: "1rem",
    height: "1rem",
  },
  style19: {
    position: "absolute",
    top: "1.75rem",
    right: "0",
    zIndex: "20",
    width: "7rem",
    borderRadius: ".5rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "#e7e5e4",
    backgroundColor: "#fff",
    padding: ".25rem",
    "--tw-shadow": "0 10px 15px -3px #0000001a, 0 4px 6px -4px #0000001a",
    boxShadow:
      "0 0 #0000, 0 0 #0000, 0 0 #0000, 0 0 #0000, 0 10px 15px -3px var(--tw-shadow-color, #0000001a), 0 4px 6px -4px var(--tw-shadow-color, #0000001a)",
  },
  style20: {
    width: "100%",
    borderRadius: ".375rem",
    paddingInline: ".5rem",
    paddingBlock: ".375rem",
    textAlign: "left",
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: "#44403c",
    backgroundColor: {
      default: null,
      ":hover": "#f5f5f4",
    },
  },
  style21: {
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    borderColor: "#e7e5e4",
    paddingInline: ".75rem",
    paddingBlock: ".625rem",
  },
  style22: {
    display: "flex",
    alignItems: "flex-end",
    gap: ".5rem",
    borderRadius: "10px",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: {
      default: "#d6d3d1",
      ":focus-within": "#a8a29e",
    },
    backgroundColor: "#fff",
    padding: ".375rem",
    paddingLeft: ".5rem",
    "--tw-ring-shadow": {
      default: null,
      ":focus-within": " 0 0 0 calc(2px + 0) currentcolor",
    },
    boxShadow: {
      default: null,
      ":focus-within":
        "0 0 #0000, 0 0 #0000, 0 0 #0000, var(--tw-ring-inset, ) 0 0 0 calc(2px + var(--tw-ring-offset-width)) var(--tw-ring-color, currentcolor), 0 0 #0000",
    },
    "--tw-ring-color": {
      default: null,
      ":focus-within": "#e7e5e4",
    },
  },
  style23: {
    maxHeight: "5rem",
    minHeight: "1.5rem",
    minWidth: "0",
    flex: "1",
    resize: "none",
    backgroundColor: "#0000",
    paddingBlock: ".125rem",
    fontSize: ".75rem",
    lineHeight: "18px",
    "--tw-leading": "18px",
    color: "#292524",
    "::placeholder": {
      default: null,
      color: "#a8a29e",
    },
    "--tw-outline-style": {
      default: null,
      ":focus": "none",
    },
    outlineStyle: {
      default: null,
      ":focus": "none",
    },
    outlineOffset: {
      default: null,
      "@media (forced-colors: active)": {
        default: null,
        ":focus": "2px",
      },
    },
    outline: {
      default: null,
      "@media (forced-colors: active)": {
        default: null,
        ":focus": "2px solid #0000",
      },
    },
  },
  style24: {
    display: "grid",
    width: "1.5rem",
    height: "1.5rem",
    flexShrink: "0",
    placeItems: "center",
    borderRadius: "3.40282e38px",
    backgroundColor: {
      default: "#1c1917",
      ":disabled": "#e7e5e4",
    },
    color: {
      default: "#fff",
      ":disabled": "#a8a29e",
    },
  },
  style25: {
    width: ".875rem",
    height: ".875rem",
  },
  style26: {
    marginTop: ".375rem",
    fontSize: "11px",
    color: "#b91c1c",
  },
});
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
  composer: {
    top: number;
  } | null;
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
    <div {...stylex.props(styles.style1)}>
      {composer ? (
        <div
          ref={measureRef(DRAFT_COMMENT_ID)}
          {...stylex.props(styles.style2)}
          style={{
            top: topById.get(DRAFT_COMMENT_ID) ?? composer.top,
          }}
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
            {...stylex.props(styles.style2)}
            style={{
              top: topById.get(thread.id) ?? 0,
            }}
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
      {...stylex.props([
        "overflow-hidden rounded-[22px] border bg-white text-left shadow-[0_7px_22px_rgba(0,0,0,0.08)]",
        "transition-[border-color,box-shadow]",
        active ? "border-stone-400 shadow-lg" : "border-stone-300",
        onActivate && "cursor-pointer",
      ])}
    >
      <div {...stylex.props(styles.style3)}>
        <CommentHeader
          canDelete={canDelete(comment)}
          comment={comment}
          deleteDisabled={deleteDisabled}
          deleting={deletingCommentId === comment.commentId}
          onDelete={onDelete}
        />
        <p {...stylex.props(styles.style4)}>{comment.body}</p>
      </div>
      {replies.length ? (
        <div
          {...stylex.props(styles.style5)}
          onClick={(event) => event.stopPropagation()}
        >
          {replies.map((reply) => (
            <div key={reply.commentId} {...stylex.props(styles.style6)}>
              <Avatar
                seed={
                  reply.isAuthor
                    ? "shared-note:you"
                    : "shared-note:collaborator"
                }
                label={reply.isAuthor ? "You" : "Collaborator"}
                size={24}
              />
              <div {...stylex.props(styles.style7)}>
                <div {...stylex.props(styles.style8)}>
                  <p {...stylex.props(styles.style9)}>
                    {reply.isAuthor ? "You" : "Collaborator"}{" "}
                    <time
                      {...stylex.props(styles.style10)}
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
                <p {...stylex.props(styles.style11)}>{reply.body}</p>
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
    <div {...stylex.props(styles.style12)}>
      <div {...stylex.props(styles.style13)}>
        <Avatar
          seed={
            comment.isAuthor ? "shared-note:you" : "shared-note:collaborator"
          }
          label={comment.isAuthor ? "You" : "Collaborator"}
          size={25}
        />
        <p {...stylex.props(styles.style14)}>
          {comment.isAuthor ? "You" : "Collaborator"}{" "}
          <time {...stylex.props(styles.style10)} dateTime={comment.createdAt}>
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
    <div {...stylex.props(styles.style15)}>
      <button
        type="button"
        aria-label="Comment actions"
        aria-expanded={open}
        {...stylex.props(styles.style16)}
        disabled={deleteDisabled}
        onClick={(event) => {
          event.stopPropagation();
          setOpen((current) => !current);
        }}
      >
        {deleting ? (
          <CircleNotch {...stylex.props(styles.style17)} aria-hidden="true" />
        ) : (
          <DotsThree {...stylex.props(styles.style18)} aria-hidden="true" />
        )}
      </button>
      {open ? (
        <div
          {...stylex.props(styles.style19)}
          onClick={(event) => event.stopPropagation()}
        >
          <button
            type="button"
            {...stylex.props(styles.style20)}
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
      {...stylex.props(styles.style21)}
      onClick={(event) => event.stopPropagation()}
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <div {...stylex.props(styles.style22)}>
        <Avatar seed="shared-note:you" label="You" size={24} />
        <textarea
          aria-label="Reply to comment"
          aria-invalid={tooLong}
          {...stylex.props(styles.style23)}
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
          {...stylex.props(styles.style24)}
          disabled={!validated.valid || pending}
        >
          {pending ? (
            <CircleNotch {...stylex.props(styles.style17)} aria-hidden="true" />
          ) : (
            <ArrowUp {...stylex.props(styles.style25)} aria-hidden="true" />
          )}
        </button>
      </div>
      {tooLong ? (
        <p {...stylex.props(styles.style26)} role="alert">
          Reply is too long ({validated.byteLength.toLocaleString()}/
          {MAX_SHARED_NOTE_COMMENT_BYTES.toLocaleString()} bytes).
        </p>
      ) : null}
      {error ? (
        <p {...stylex.props(styles.style26)} role="status">
          Your reply couldn’t be added. Try again.
        </p>
      ) : null}
    </form>
  );
}
