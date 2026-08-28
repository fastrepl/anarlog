import { Trans, useLingui } from "@lingui/react/macro";
import { CircleNotch, Trash } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { EditorView } from "prosemirror-view";
import { useCallback, useRef, useState } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";
import {
  captureCommentAnchor,
  type CommentAnchor,
} from "@anlg/editor/comments";
import {
  type CommentAnchorsEvent,
  getCommentAnchorRanges,
  setActiveCommentAnchor,
} from "@anlg/editor/note";
import { Avatar } from "@anlg/ui/components/avatar";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@anlg/ui/components/ui/popover";
import { Textarea } from "@anlg/ui/components/ui/textarea";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import {
  createSessionShareComment,
  deleteSessionShareComment,
  listSessionShareComments,
  type SessionShareComment,
  type SessionShareCommentAnchor,
  type SessionShareCommentPage,
  ShareManagementError,
} from "./client";
import {
  applySessionShareCommentAnchors,
  sessionShareCommentsQueryKey,
} from "./comment-anchors";

import { trackAnalyticsEvent } from "~/analytics";
import { useAuth } from "~/auth";
import { loadManagedSharedNoteForSession } from "~/shared-notes/cache";

const MAX_COMMENT_BODY_BYTES = 16_384;
const EMPTY_COMMENTS: SessionShareComment[] = [];

type CommentPosition = {
  left: number;
  top: number;
};

type SessionCommentsController = {
  containerRef: React.RefObject<HTMLDivElement | null>;
  comments: SessionShareComment[];
  createError: boolean;
  createPending: boolean;
  deletePending: boolean;
  deletingCommentId: string | null;
  draft:
    | (CommentPosition & { anchor: CommentAnchor; from: number; to: number })
    | null;
  manageAccess: boolean;
  openAnchor: (CommentPosition & { commentIds: string[]; pos: number }) | null;
  selection:
    | (CommentPosition & { anchor: CommentAnchor; from: number; to: number })
    | null;
  anchorSyncRef: (element: HTMLSpanElement | null) => void;
  cancelDraft: () => void;
  closeComments: () => void;
  deleteComment: (commentId: string) => void;
  onCommentAnchorsEvent: (event: CommentAnchorsEvent) => void;
  onViewDisposed: (view: EditorView) => void;
  onViewReady: (view: EditorView) => void;
  startDraft: () => void;
  submitDraft: (body: string) => void;
};

export function useOwnedSessionComments(sessionId: string) {
  const auth = useAuth();
  const session =
    auth.session && auth.session.user.is_anonymous !== true
      ? auth.session
      : null;
  const userId = session?.user.id ?? null;
  const shareQuery = useQuery({
    queryKey: ["session-managed-share", userId ?? "", sessionId],
    queryFn: () => loadManagedSharedNoteForSession(userId!, sessionId),
    enabled: userId !== null,
  });

  return useSessionComments({
    canCompose: shareQuery.data !== null && shareQuery.data !== undefined,
    currentRevision: shareQuery.data?.contentRevision ?? -1,
    manageAccess: true,
    shareId: shareQuery.data?.shareId ?? null,
  });
}

export function useSharedSessionComments({
  canCompose,
  currentRevision,
  manageAccess,
  shareId,
}: {
  canCompose: boolean;
  currentRevision: number;
  manageAccess: boolean;
  shareId: string | null;
}) {
  return useSessionComments({
    canCompose,
    currentRevision,
    manageAccess,
    shareId,
  });
}

function useSessionComments({
  canCompose,
  currentRevision,
  manageAccess,
  shareId,
}: {
  canCompose: boolean;
  currentRevision: number;
  manageAccess: boolean;
  shareId: string | null;
}): SessionCommentsController {
  const auth = useAuth();
  const queryClient = useQueryClient();
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [view, setView] = useState<EditorView | null>(null);
  const [selection, setSelection] = useState<
    | (CommentPosition & {
        anchor: CommentAnchor;
        from: number;
        to: number;
      })
    | null
  >(null);
  const [draft, setDraft] = useState<
    | (CommentPosition & {
        anchor: CommentAnchor;
        from: number;
        to: number;
      })
    | null
  >(null);
  const [openAnchor, setOpenAnchor] = useState<
    (CommentPosition & { commentIds: string[]; pos: number }) | null
  >(null);
  const session =
    auth.session && auth.session.user.is_anonymous !== true
      ? auth.session
      : null;
  const supabase = auth.supabase;
  const enabled =
    shareId !== null &&
    session !== null &&
    supabase !== null &&
    supabase !== undefined;

  const commentsQuery = useQuery({
    queryKey: sessionShareCommentsQueryKey(shareId ?? ""),
    queryFn: ({ signal }) => {
      if (!supabase || !session || !shareId) {
        throw new ShareManagementError();
      }
      return listSessionShareComments(
        { supabase, session, signal },
        { shareId },
      );
    },
    enabled,
  });
  const comments = commentsQuery.data?.comments ?? EMPTY_COMMENTS;

  const createMutation = useMutation({
    mutationFn: ({
      anchor,
      body,
    }: {
      anchor: SessionShareCommentAnchor;
      body: string;
    }) => {
      if (!supabase || !session || !shareId) {
        throw new ShareManagementError();
      }
      return createSessionShareComment(
        { supabase, session },
        { anchor, body, shareId },
      );
    },
    onSuccess: (comment) => {
      if (!shareId) return;
      trackAnalyticsEvent("share_collaboration_performed", {
        action: "comment_created",
        actor_role: manageAccess ? "owner" : "recipient",
        anchor_type: comment.anchor ? "selection" : "note",
      });
      queryClient.setQueryData<SessionShareCommentPage>(
        sessionShareCommentsQueryKey(shareId),
        (current) => ({
          comments: [...(current?.comments ?? []), comment],
          nextCursor: current?.nextCursor ?? null,
        }),
      );
      setDraft(null);
      setOpenAnchor(null);
      if (view) setActiveCommentAnchor(view, null);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (commentId: string) => {
      if (!supabase || !session) {
        throw new ShareManagementError();
      }
      return deleteSessionShareComment({ supabase, session }, commentId);
    },
    onSuccess: ({ commentId }) => {
      if (!shareId) return;
      queryClient.setQueryData<SessionShareCommentPage>(
        sessionShareCommentsQueryKey(shareId),
        (current) =>
          current
            ? {
                ...current,
                comments: current.comments.filter(
                  (comment) => comment.commentId !== commentId,
                ),
              }
            : current,
      );
      setOpenAnchor((current) => {
        if (!current) return null;
        const commentIds = current.commentIds.filter((id) => id !== commentId);
        return commentIds.length > 0 ? { ...current, commentIds } : null;
      });
    },
  });

  const positionAt = useCallback(
    (pos: number): CommentPosition | null => {
      const container = containerRef.current;
      if (!view || !container) return null;
      const coords = view.coordsAtPos(pos);
      const containerRect = container.getBoundingClientRect();
      return {
        left: coords.left - containerRect.left,
        top: coords.bottom - containerRect.top,
      };
    },
    [view],
  );

  const onCommentAnchorsEvent = useCallback(
    (event: CommentAnchorsEvent) => {
      if (!view) return;
      if (event.type === "selection") {
        if (!canCompose || !enabled || event.empty) {
          setSelection(null);
          return;
        }
        const anchor = captureCommentAnchor(
          view.state.doc,
          event.from,
          event.to,
          currentRevision,
        );
        const position = anchor ? positionAt(event.to) : null;
        setSelection(
          anchor && position
            ? { ...position, anchor, from: event.from, to: event.to }
            : null,
        );
        return;
      }
      if (event.type === "anchor-click") {
        if (event.commentIds.includes("draft")) return;
        const position = positionAt(event.pos);
        if (!position) return;
        setSelection(null);
        setDraft(null);
        setOpenAnchor({
          ...position,
          commentIds: event.commentIds,
          pos: event.pos,
        });
        setActiveCommentAnchor(view, event.commentIds[0] ?? null);
        return;
      }
      setSelection((current) => {
        const position = current ? positionAt(current.to) : null;
        return current && position ? reposition(current, position) : current;
      });
      setDraft((current) => {
        if (!current) return null;
        const mapped = getCommentAnchorRanges(view.state).find(
          (anchor) => anchor.commentId === "draft",
        );
        const position = positionAt(mapped?.to ?? current.to);
        return position
          ? reposition(
              {
                ...current,
                from: mapped?.from ?? current.from,
                to: mapped?.to ?? current.to,
              },
              position,
            )
          : current;
      });
      setOpenAnchor((current) => {
        const position = current ? positionAt(current.pos) : null;
        return current && position ? reposition(current, position) : current;
      });
    },
    [canCompose, currentRevision, enabled, positionAt, view],
  );

  const anchorSyncRef = useCallback(
    (element: HTMLSpanElement | null) => {
      if (!element || !view) return;
      applySessionShareCommentAnchors(view, comments, currentRevision, draft);
    },
    [comments, currentRevision, draft, view],
  );

  return {
    containerRef,
    comments,
    createError: createMutation.isError,
    createPending: createMutation.isPending,
    deletePending: deleteMutation.isPending,
    deletingCommentId: deleteMutation.variables ?? null,
    draft,
    manageAccess,
    openAnchor,
    selection,
    anchorSyncRef,
    cancelDraft: () => {
      createMutation.reset();
      setDraft(null);
      if (view) setActiveCommentAnchor(view, null);
    },
    closeComments: () => {
      setOpenAnchor(null);
      if (view) setActiveCommentAnchor(view, null);
    },
    deleteComment: (commentId) => deleteMutation.mutate(commentId),
    onCommentAnchorsEvent,
    onViewDisposed: (disposedView) => {
      setView((current) => (current === disposedView ? null : current));
      setSelection(null);
      setDraft(null);
      setOpenAnchor(null);
    },
    onViewReady: (readyView) => {
      setView(readyView);
      applySessionShareCommentAnchors(
        readyView,
        comments,
        currentRevision,
        draft,
      );
    },
    startDraft: () => {
      if (!selection) return;
      createMutation.reset();
      setDraft(selection);
      setSelection(null);
      setOpenAnchor(null);
      if (view) setActiveCommentAnchor(view, null);
    },
    submitDraft: (body) => {
      if (!draft || createMutation.isPending) return;
      createMutation.mutate({
        anchor: {
          quoteExact: draft.anchor.quoteExact,
          quotePrefix: draft.anchor.quotePrefix,
          quoteSuffix: draft.anchor.quoteSuffix,
          fromHint: draft.anchor.fromHint,
          toHint: draft.anchor.toHint,
        },
        body,
      });
    },
  };
}

export function SessionCommentsLayer({
  controller,
}: {
  controller: SessionCommentsController;
}) {
  const openComments = controller.openAnchor
    ? controller.comments.filter((comment) =>
        controller.openAnchor?.commentIds.includes(comment.commentId),
      )
    : [];

  return (
    <>
      <span ref={controller.anchorSyncRef} {...stylex.props(styles.hidden)} />
      {controller.draft && (
        <Popover
          open
          onOpenChange={(open) => {
            if (!open) controller.cancelDraft();
          }}
        >
          <CommentPopoverAnchor position={controller.draft} />
          <PopoverContent
            align="start"
            side="bottom"
            sx={styles.composerPopover}
          >
            <CommentComposer
              error={controller.createError}
              pending={controller.createPending}
              onCancel={controller.cancelDraft}
              onSubmit={controller.submitDraft}
            />
          </PopoverContent>
        </Popover>
      )}
      {controller.openAnchor && openComments.length > 0 && (
        <Popover
          open
          onOpenChange={(open) => {
            if (!open) controller.closeComments();
          }}
        >
          <CommentPopoverAnchor position={controller.openAnchor} />
          <PopoverContent
            align="start"
            side="bottom"
            sx={styles.commentsPopover}
          >
            {openComments.map((comment) => (
              <SessionCommentItem
                key={comment.commentId}
                comment={comment}
                deleting={
                  controller.deletePending &&
                  controller.deletingCommentId === comment.commentId
                }
                deleteDisabled={controller.deletePending}
                onDelete={() => controller.deleteComment(comment.commentId)}
                showDelete={comment.isAuthor || controller.manageAccess}
              />
            ))}
          </PopoverContent>
        </Popover>
      )}
    </>
  );
}

function CommentPopoverAnchor({ position }: { position: CommentPosition }) {
  return (
    <PopoverAnchor asChild>
      <span
        aria-hidden="true"
        {...mergeStyleXProps(styles.anchor, undefined, {
          left: position.left,
          top: position.top,
        })}
      />
    </PopoverAnchor>
  );
}

function CommentComposer({
  error,
  onCancel,
  onSubmit,
  pending,
}: {
  error: boolean;
  onCancel: () => void;
  onSubmit: (body: string) => void;
  pending: boolean;
}) {
  const { t } = useLingui();
  const form = useForm({
    defaultValues: { body: "" },
    onSubmit: ({ value }) => {
      const comment = validateComment(value.body);
      if (comment.valid) onSubmit(comment.body);
    },
  });

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        if (pending) return;
        void form.handleSubmit();
      }}
    >
      <form.Field name="body">
        {(field) => {
          const comment = validateComment(field.state.value);
          return (
            <>
              <div {...stylex.props(styles.composerRow)}>
                <Avatar
                  seed="shared-note:you"
                  label={t`You`}
                  size={28}
                  sx={styles.composerAvatar}
                />
                <Textarea
                  autoFocus
                  aria-label={t`Comment on selected text`}
                  aria-invalid={comment.tooLong}
                  sx={styles.textarea}
                  placeholder={t`Comment on the selected text…`}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Escape") {
                      event.preventDefault();
                      onCancel();
                    }
                    if (
                      event.key === "Enter" &&
                      (event.metaKey || event.ctrlKey)
                    ) {
                      event.preventDefault();
                      void form.handleSubmit();
                    }
                  }}
                />
              </div>
              {comment.tooLong && (
                <p {...stylex.props(styles.error)} role="alert">
                  <Trans>Comment is too long.</Trans>
                </p>
              )}
              {error && (
                <p {...stylex.props(styles.error)} role="status">
                  <Trans>Your comment couldn’t be added. Try again.</Trans>
                </p>
              )}
              <div {...stylex.props(styles.composerActions)}>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  disabled={pending}
                  onClick={onCancel}
                >
                  <Trans>Cancel</Trans>
                </Button>
                <Button
                  type="submit"
                  size="sm"
                  disabled={pending || !comment.valid}
                >
                  {pending && (
                    <CircleNotch
                      {...stylex.props(styles.spinner)}
                      aria-hidden="true"
                    />
                  )}
                  <Trans>Comment</Trans>
                </Button>
              </div>
            </>
          );
        }}
      </form.Field>
    </form>
  );
}

function SessionCommentItem({
  comment,
  deleteDisabled,
  deleting,
  onDelete,
  showDelete,
}: {
  comment: SessionShareComment;
  deleteDisabled: boolean;
  deleting: boolean;
  onDelete: () => void;
  showDelete: boolean;
}) {
  const { t } = useLingui();

  return (
    <div {...stylex.props(styles.comment)}>
      <div {...stylex.props(styles.commentHeader)}>
        <div {...stylex.props(styles.author)}>
          <Avatar
            seed={
              comment.isAuthor ? "shared-note:you" : "shared-note:collaborator"
            }
            label={comment.isAuthor ? t`You` : t`Collaborator`}
            size={24}
          />
          <span {...stylex.props(styles.authorName)}>
            {comment.isAuthor ? (
              <Trans>You</Trans>
            ) : (
              <Trans>Collaborator</Trans>
            )}
          </span>
        </div>
        {showDelete && (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            sx={styles.deleteButton}
            aria-label={t`Delete comment`}
            disabled={deleteDisabled}
            onClick={onDelete}
          >
            {deleting ? (
              <CircleNotch
                {...stylex.props(styles.spinner)}
                aria-hidden="true"
              />
            ) : (
              <Trash {...stylex.props(styles.smallIcon)} aria-hidden="true" />
            )}
          </Button>
        )}
      </div>
      <p {...stylex.props(styles.commentBody)}>{comment.body}</p>
    </div>
  );
}

function validateComment(value: string) {
  const body = value.trim();
  const byteLength = new TextEncoder().encode(body).length;
  const tooLong = byteLength > MAX_COMMENT_BODY_BYTES;
  return {
    body,
    tooLong,
    valid: body.length > 0 && !tooLong,
  };
}

function reposition<T extends CommentPosition>(
  current: T,
  next: CommentPosition,
) {
  return current.left === next.left && current.top === next.top
    ? current
    : { ...current, ...next };
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  anchor: {
    height: 0,
    pointerEvents: "none",
    position: "absolute",
    width: 0,
  },
  author: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    minWidth: 0,
  },
  authorName: {
    fontSize: "0.875rem",
    fontWeight: 500,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  comment: {
    borderBottomColor: {
      default: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
      ":last-child": "transparent",
    },
    borderBottomStyle: "solid",
    borderBottomWidth: {
      default: "1px",
      ":last-child": 0,
    },
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
  },
  commentBody: {
    fontSize: "0.875rem",
    marginTop: "0.375rem",
    whiteSpace: "pre-wrap",
  },
  commentHeader: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    justifyContent: "space-between",
  },
  commentsPopover: {
    maxHeight: "20rem",
    overflowY: "auto",
    padding: 0,
    width: "20rem",
  },
  composerActions: {
    display: "flex",
    gap: "0.5rem",
    justifyContent: "flex-end",
    marginTop: "0.75rem",
  },
  composerAvatar: {
    marginTop: "0.25rem",
  },
  composerPopover: {
    padding: "0.75rem",
    width: "20rem",
  },
  composerRow: {
    alignItems: "flex-start",
    display: "flex",
    gap: "0.5rem",
  },
  deleteButton: {
    color: colors.mutedForeground,
    height: "1.5rem",
    width: "1.5rem",
  },
  error: {
    color: colors.destructive,
    fontSize: "0.75rem",
    marginTop: "0.5rem",
  },
  hidden: {
    display: "none",
  },
  smallIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "0.875rem",
    width: "0.875rem",
  },
  textarea: {
    flex: "1",
    minHeight: "5rem",
    minWidth: 0,
    resize: "none",
  },
});
