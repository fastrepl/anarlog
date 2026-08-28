import {
  Chat,
  Check,
  CircleNotch,
  Clock,
  SignIn,
  Trash,
  X,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import {
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";
import { Avatar } from "@anlg/ui/components/avatar";

import {
  useDeleteSharedNoteComment,
  useSharedNoteComments,
} from "@/components/shared-note-comments-data";
import { sharedButtonStyles } from "@/components/shared-note-viewer";
import {
  cancelMySharedNoteAccessRequest,
  getMySharedNoteAccessRequest,
  listSharedNoteManagerAccess,
  requestSharedNoteCommentAccess,
  reviewSharedNoteAccessRequest,
} from "@/functions/shared-notes";
import {
  canComposeSharedNoteComments,
  formatSharedNoteAccessRequestDescription,
  hasSharedNoteCollaborationAccess,
  truncateSharedNoteCommentQuote,
} from "@/lib/shared-note-collaboration";
import type {
  SessionAccessRequestState,
  SessionShareAccessCursor,
  SharedNoteCapability,
  SharedNoteComment,
} from "@/lib/shared-notes";

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  style1: {
    marginTop: "1.5rem",
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: "1.5rem",
    borderStyle: "solid",
    borderWidth: "1px",
    paddingInline: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2.5rem",
    },
    paddingBlock: "1.75rem",
    boxShadow: "0 1px 2px 0 #0000000d",
  },
  style2: {
    display: "flex",
    alignItems: "flex-start",
    justifyContent: "space-between",
    gap: "1.25rem",
  },
  style3: {
    display: "flex",
    alignItems: "center",
    gap: ".5rem",
    color: colors.foreground,
  },
  style4: {
    width: "1.25rem",
    height: "1.25rem",
  },
  style5: {
    color: colors.foreground,
    fontFamily: fonts.mono,
    fontSize: "1.125rem",
    lineHeight: "1.75rem",
    fontWeight: 500,
  },
  style6: {
    marginTop: ".25rem",
    color: colors.mutedForeground,
    fontSize: ".875rem",
    lineHeight: "1.5rem",
  },
  style7: {
    backgroundColor: colors.muted,
    borderRadius: radii.full,
    paddingInline: ".75rem",
    paddingBlock: ".25rem",
    color: colors.mutedForeground,
    fontFamily: fonts.mono,
    fontSize: ".75rem",
    lineHeight: "1rem",
  },
  style8: {
    marginTop: "1.5rem",
    backgroundColor: colors.muted,
    borderRadius: "1rem",
    paddingInline: "1rem",
    paddingBlock: "1rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: colors.mutedForeground,
  },
  style9: {
    marginTop: "1.5rem",
    borderColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    paddingTop: "1.25rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: colors.mutedForeground,
  },
  style10: {
    marginTop: "1.5rem",
    display: "flex",
    alignItems: "center",
    gap: ".5rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: colors.mutedForeground,
  },
  style11: {
    width: "1rem",
    height: "1rem",
    animationDuration: "1s",
    animationTimingFunction: "linear",
    animationIterationCount: "infinite",
    animationName: spin,
  },
  style12: {
    marginTop: "1.5rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#b91c1c",
  },
  style13: {
    marginTop: ".75rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#b91c1c",
  },
  style14: {
    marginTop: "1.5rem",
    borderBottomColor: {
      ":is(*) > :not(:last-child)": colors.border,
    },
    borderBottomStyle: {
      ":is(*) > :not(:last-child)": "solid",
    },
    borderBottomWidth: {
      ":is(*) > :not(:last-child)": "1px",
    },
    borderColor: colors.border,
    borderBlockStyle: "solid",
    borderBlockWidth: "1px",
  },
  style15: {
    paddingBlock: "1.25rem",
  },
  style16: {
    display: "flex",
    alignItems: "flex-start",
    justifyContent: "space-between",
    gap: "1rem",
  },
  style17: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    gap: ".625rem",
  },
  style18: {
    color: colors.foreground,
    fontFamily: fonts.mono,
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
  },
  style19: {
    marginTop: ".125rem",
    display: "block",
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: colors.mutedForeground,
  },
  style20: {
    borderRadius: radii.full,
    padding: ".5rem",
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
    boxShadow: {
      default: null,
      ":focus-visible": "0 0 0 2px #78716c",
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
    opacity: {
      default: null,
      ":disabled": 0.5,
    },
  },
  style21: {
    width: "1rem",
    height: "1rem",
  },
  style22: {
    marginTop: ".75rem",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    overflow: "hidden",
    borderLeftStyle: "solid",
    borderLeftWidth: "2px",
    borderColor: colors.border,
    paddingLeft: ".75rem",
    fontSize: ".75rem",
    lineHeight: "1.25rem",
    color: colors.mutedForeground,
  },
  style23: {
    marginTop: ".75rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    whiteSpace: "pre-wrap",
    color: colors.foreground,
  },
  style24: {
    marginTop: "1.5rem",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    paddingInline: "1rem",
    paddingBlock: "1.25rem",
    display: {
      default: null,
      "@media (width >= 40rem)": "flex",
    },
    alignItems: {
      default: null,
      "@media (width >= 40rem)": "center",
    },
    justifyContent: {
      default: null,
      "@media (width >= 40rem)": "space-between",
    },
    gap: {
      default: null,
      "@media (width >= 40rem)": "1.25rem",
    },
  },
  style25: {
    marginRight: ".5rem",
    width: "1rem",
    height: "1rem",
  },
  style26: {
    marginTop: "1.5rem",
    display: "flex",
    alignItems: "center",
    gap: ".5rem",
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    borderColor: colors.border,
    paddingTop: "1.25rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: colors.mutedForeground,
  },
  style27: {
    marginTop: "1.5rem",
    borderColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    paddingTop: "1.25rem",
  },
  style28: {
    display: {
      default: null,
      "@media (width >= 40rem)": "flex",
    },
    alignItems: {
      default: null,
      "@media (width >= 40rem)": "center",
    },
    justifyContent: {
      default: null,
      "@media (width >= 40rem)": "space-between",
    },
    gap: {
      default: null,
      "@media (width >= 40rem)": "1.25rem",
    },
  },
  style29: {
    display: "flex",
    alignItems: "center",
    gap: ".5rem",
    color: colors.foreground,
    fontFamily: fonts.mono,
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
  },
  style30: {
    marginTop: {
      default: "1rem",
      "@media (width >= 40rem)": 0,
    },
    display: "flex",
    flexShrink: 0,
    gap: ".5rem",
  },
  style31: {
    marginTop: {
      default: ".75rem",
      ":is(*) > :not(:first-child)": ".5rem",
    },
  },
  style32: {
    backgroundColor: colors.muted,
    borderRadius: "1rem",
    paddingInline: "1rem",
    paddingBlock: ".75rem",
    display: {
      default: null,
      "@media (width >= 40rem)": "flex",
    },
    alignItems: {
      default: null,
      "@media (width >= 40rem)": "center",
    },
    justifyContent: {
      default: null,
      "@media (width >= 40rem)": "space-between",
    },
    gap: {
      default: null,
      "@media (width >= 40rem)": "1rem",
    },
  },
  style33: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: colors.foreground,
  },
  style34: {
    marginTop: ".125rem",
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: colors.mutedForeground,
  },
  style35: {
    marginTop: {
      default: ".75rem",
      "@media (width >= 40rem)": 0,
    },
    display: "flex",
    gap: ".5rem",
  },
  style36: {
    marginRight: ".375rem",
    width: "1rem",
    height: "1rem",
  },
  style37: {
    marginRight: ".375rem",
    width: "1rem",
    height: "1rem",
    animationDuration: "1s",
    animationTimingFunction: "linear",
    animationIterationCount: "infinite",
    animationName: spin,
  },
  style38: {
    marginRight: ".5rem",
    width: "1rem",
    height: "1rem",
    animationDuration: "1s",
    animationTimingFunction: "linear",
    animationIterationCount: "infinite",
    animationName: spin,
  },
  loadEarlierComments: {
    marginTop: "1.5rem",
  },
  signInButton: {
    marginTop: {
      default: "1rem",
      "@media (width >= 40rem)": 0,
    },
  },
  compactButton: {
    minHeight: "2.25rem",
    paddingInline: ".75rem",
  },
  loadEarlierRequests: {
    marginTop: ".75rem",
  },
});
const accessRequestQueryKey = (shareId: string) => [
  "shared-note-access-request",
  shareId,
];
const managerAccessQueryKey = (shareId: string) => [
  "shared-note-manager-access",
  shareId,
];
export function SharedNoteCollaboration({
  capability,
  currentUserId,
  manageAccess,
  returnPath,
  shareId,
}: {
  capability: SharedNoteCapability;
  currentUserId: string | null;
  manageAccess: boolean;
  returnPath: string;
  shareId: string;
}) {
  const queryClient = useQueryClient();
  const signedIn = currentUserId !== null;
  const commentsQuery = useSharedNoteComments({
    enabled: signedIn,
    shareId,
  });
  const accessRequestQuery = useQuery({
    queryKey: accessRequestQueryKey(shareId),
    queryFn: async () => {
      const result = await getMySharedNoteAccessRequest({
        data: shareId,
      });
      if (result.status !== "ready") {
        throw new Error("access request unavailable");
      }
      return result.request;
    },
    enabled: signedIn && !manageAccess,
    retry: false,
  });
  const managerAccessQuery = useInfiniteQuery({
    queryKey: managerAccessQueryKey(shareId),
    queryFn: async ({ pageParam }) => {
      const result = await listSharedNoteManagerAccess({
        data: {
          shareId,
          beforeCreatedAt: pageParam?.beforeCreatedAt ?? null,
          beforeEntryId: pageParam?.beforeEntryId ?? null,
        },
      });
      if (result.status !== "ready") {
        throw new Error("manager access unavailable");
      }
      return result;
    },
    enabled: signedIn && manageAccess,
    initialPageParam: null as SessionShareAccessCursor | null,
    getNextPageParam: (lastPage) => lastPage.nextCursor ?? undefined,
    retry: false,
  });
  const deleteMutation = useDeleteSharedNoteComment({
    shareId,
  });
  const requestMutation = useMutation({
    mutationFn: async () => {
      const result = await requestSharedNoteCommentAccess({
        data: shareId,
      });
      if (result.status !== "ready") {
        throw new Error("access request unavailable");
      }
      return result.request;
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: accessRequestQueryKey(shareId),
      });
    },
  });
  const cancelRequestMutation = useMutation({
    mutationFn: async (requestId: string) => {
      const result = await cancelMySharedNoteAccessRequest({
        data: requestId,
      });
      if (result.status !== "ready") {
        throw new Error("access request unavailable");
      }
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: accessRequestQueryKey(shareId),
      });
    },
  });
  const reviewMutation = useMutation({
    mutationFn: async ({
      decision,
      capability,
      requestId,
    }: {
      decision: "approved" | "denied";
      capability: SharedNoteCapability;
      requestId: string;
    }) => {
      const result = await reviewSharedNoteAccessRequest({
        data:
          decision === "approved"
            ? {
                capability,
                decision,
                requestId,
              }
            : {
                decision,
                requestId,
              },
      });
      if (result.status !== "ready") {
        throw new Error("access request unavailable");
      }
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: managerAccessQueryKey(shareId),
      });
    },
  });
  const hasCollaborationAccess = hasSharedNoteCollaborationAccess(
    commentsQuery.data?.pages[0],
  );
  const canCompose = canComposeSharedNoteComments({
    capability,
    hasCollaborationAccess,
    manageAccess,
  });
  const comments = [...(commentsQuery.data?.pages ?? [])]
    .reverse()
    .flatMap((page) => (page.status === "ready" ? page.comments : []));
  const accessRequest = accessRequestQuery.data ?? null;
  const pendingManagerRequests = (managerAccessQuery.data?.pages ?? [])
    .flatMap((page) => page.entries)
    .filter(
      (entry) => entry.entryType === "request" && entry.status === "pending",
    );
  return (
    <section
      aria-labelledby="shared-note-comments-heading"
      {...stylex.props(styles.style1)}
    >
      <div {...stylex.props(styles.style2)}>
        <div>
          <div {...stylex.props(styles.style3)}>
            <Chat {...stylex.props(styles.style4)} aria-hidden="true" />
            <h2
              id="shared-note-comments-heading"
              {...stylex.props(styles.style5)}
            >
              Comments
            </h2>
          </div>
          <p {...stylex.props(styles.style6)}>
            A private conversation for people with access to this note.
          </p>
        </div>
        {hasCollaborationAccess && commentsQuery.isSuccess && (
          <span {...stylex.props(styles.style7)}>
            {comments.length}
            {commentsQuery.hasNextPage ? "+" : ""}
          </span>
        )}
      </div>

      {!signedIn ? (
        <SignInToCollaborate returnPath={returnPath} />
      ) : (
        <>
          {commentsQuery.isPending ||
          commentsQuery.isError ||
          hasCollaborationAccess ? (
            <CommentList
              comments={comments}
              error={
                (commentsQuery.isError &&
                  !commentsQuery.isFetchNextPageError) ||
                deleteMutation.isError
              }
              hasEarlier={commentsQuery.hasNextPage}
              loading={commentsQuery.isPending}
              loadingEarlier={commentsQuery.isFetchingNextPage}
              loadEarlierError={commentsQuery.isFetchNextPageError}
              manageAccess={manageAccess}
              deletingCommentId={deleteMutation.variables ?? null}
              deletePending={deleteMutation.isPending}
              onDelete={(commentId) => deleteMutation.mutate(commentId)}
              onLoadEarlier={() => void commentsQuery.fetchNextPage()}
            />
          ) : (
            <p {...stylex.props(styles.style8)}>
              Comments are available after the note owner grants your account
              access.
            </p>
          )}

          {canCompose && (
            <p {...stylex.props(styles.style9)}>
              Select text in the note to comment. Comments are visible only to
              people who can open this note.
            </p>
          )}

          {!canCompose &&
            !manageAccess &&
            !commentsQuery.isPending &&
            !commentsQuery.isError && (
              <AccessRequestPanel
                request={accessRequest}
                error={
                  accessRequestQuery.isError ||
                  requestMutation.isError ||
                  cancelRequestMutation.isError
                }
                loading={accessRequestQuery.isPending}
                pending={
                  requestMutation.isPending || cancelRequestMutation.isPending
                }
                onCancel={(requestId) =>
                  cancelRequestMutation.mutate(requestId)
                }
                onRequest={() => requestMutation.mutate()}
              />
            )}

          {manageAccess && (
            <ManagerRequests
              error={
                managerAccessQuery.isError ||
                managerAccessQuery.isFetchNextPageError ||
                reviewMutation.isError
              }
              hasEarlierRequests={managerAccessQuery.hasNextPage}
              loading={managerAccessQuery.isPending}
              loadingEarlier={managerAccessQuery.isFetchingNextPage}
              pendingRequestId={
                reviewMutation.isPending
                  ? (reviewMutation.variables?.requestId ?? null)
                  : null
              }
              requests={pendingManagerRequests}
              onLoadEarlier={() => managerAccessQuery.fetchNextPage()}
              onReview={(requestId, decision, capability) =>
                reviewMutation.mutate({
                  capability,
                  decision,
                  requestId,
                })
              }
            />
          )}
        </>
      )}
    </section>
  );
}
function CommentList({
  comments,
  deletePending,
  deletingCommentId,
  error,
  hasEarlier,
  loading,
  loadingEarlier,
  loadEarlierError,
  manageAccess,
  onDelete,
  onLoadEarlier,
}: {
  comments: SharedNoteComment[];
  deletePending: boolean;
  deletingCommentId: string | null;
  error: boolean;
  hasEarlier: boolean;
  loading: boolean;
  loadingEarlier: boolean;
  loadEarlierError: boolean;
  manageAccess: boolean;
  onDelete: (commentId: string) => void;
  onLoadEarlier: () => void;
}) {
  if (loading) {
    return (
      <div {...stylex.props(styles.style10)}>
        <CircleNotch {...stylex.props(styles.style11)} aria-hidden="true" />
        Loading comments…
      </div>
    );
  }
  if (error) {
    return (
      <p {...stylex.props(styles.style12)} role="status">
        Comments couldn’t be loaded right now.
      </p>
    );
  }
  if (!comments.length) {
    return <p {...stylex.props(styles.style8)}>No comments yet.</p>;
  }
  return (
    <>
      {hasEarlier && (
        <button
          type="button"
          {...stylex.props(
            sharedButtonStyles.base,
            sharedButtonStyles.secondary,
            styles.loadEarlierComments,
          )}
          disabled={loadingEarlier}
          onClick={onLoadEarlier}
        >
          {loadingEarlier && (
            <CircleNotch {...stylex.props(styles.style11)} aria-hidden="true" />
          )}
          Load earlier comments
        </button>
      )}
      {loadEarlierError && (
        <p {...stylex.props(styles.style13)} role="status">
          Earlier comments couldn’t be loaded. Please try again.
        </p>
      )}
      <ol {...stylex.props(styles.style14)}>
        {comments.map((comment) => {
          const canDelete = comment.isAuthor || manageAccess;
          const deleting =
            deletePending && deletingCommentId === comment.commentId;
          return (
            <li
              key={comment.commentId}
              id={`shared-comment-${comment.commentId}`}
              {...stylex.props(styles.style15)}
            >
              <div {...stylex.props(styles.style16)}>
                <div {...stylex.props(styles.style17)}>
                  <Avatar
                    seed={
                      comment.isAuthor
                        ? "shared-note:you"
                        : "shared-note:collaborator"
                    }
                    label={comment.isAuthor ? "You" : "Collaborator"}
                    size={28}
                  />
                  <div>
                    <p {...stylex.props(styles.style18)}>
                      {comment.isAuthor ? "You" : "Collaborator"}
                    </p>
                    <time
                      {...stylex.props(styles.style19)}
                      dateTime={comment.createdAt}
                    >
                      {formatCommentDate(comment.createdAt)}
                    </time>
                  </div>
                </div>
                {canDelete && (
                  <button
                    type="button"
                    {...stylex.props(styles.style20)}
                    aria-label="Delete comment"
                    disabled={deletePending}
                    onClick={() => onDelete(comment.commentId)}
                  >
                    {deleting ? (
                      <CircleNotch
                        {...stylex.props(styles.style11)}
                        aria-hidden="true"
                      />
                    ) : (
                      <Trash
                        {...stylex.props(styles.style21)}
                        aria-hidden="true"
                      />
                    )}
                  </button>
                )}
              </div>
              {comment.anchor && (
                <p {...stylex.props(styles.style22)}>
                  {truncateSharedNoteCommentQuote(comment.anchor.quoteExact)}
                </p>
              )}
              <p {...stylex.props(styles.style23)}>{comment.body}</p>
            </li>
          );
        })}
      </ol>
    </>
  );
}
function SignInToCollaborate({ returnPath }: { returnPath: string }) {
  const search = new URLSearchParams({
    flow: "web",
    redirect: returnPath,
  });
  return (
    <div {...stylex.props(styles.style24)}>
      <div>
        <p {...stylex.props(styles.style18)}>
          Sign in to join the conversation
        </p>
        <p {...stylex.props(styles.style6)}>
          Sign in to view comments or request permission to comment.
        </p>
      </div>
      <a
        href={`/auth/?${search.toString()}`}
        {...stylex.props(
          sharedButtonStyles.base,
          sharedButtonStyles.primary,
          styles.signInButton,
        )}
      >
        <SignIn {...stylex.props(styles.style25)} aria-hidden="true" />
        Sign in
      </a>
    </div>
  );
}
function AccessRequestPanel({
  error,
  loading,
  onCancel,
  onRequest,
  pending,
  request,
}: {
  error: boolean;
  loading: boolean;
  onCancel: (requestId: string) => void;
  onRequest: () => void;
  pending: boolean;
  request: SessionAccessRequestState | null;
}) {
  if (loading) {
    return (
      <div {...stylex.props(styles.style26)}>
        <CircleNotch {...stylex.props(styles.style11)} aria-hidden="true" />
        Checking comment access…
      </div>
    );
  }
  const isPending = request?.status === "pending";
  const isApproved = request?.status === "approved";
  const description = isPending
    ? "The note owner can approve or decline your request."
    : isApproved
      ? "Your request was approved. Reload this note to use your new access."
      : request?.status === "denied"
        ? "Your previous request was declined. You can send a new request if needed."
        : request?.status === "cancelled"
          ? "Your previous request was cancelled."
          : "Ask the note owner for permission to join the conversation.";
  return (
    <div {...stylex.props(styles.style27)}>
      <div {...stylex.props(styles.style28)}>
        <div>
          <p {...stylex.props(styles.style29)}>
            {isPending && (
              <Clock {...stylex.props(styles.style21)} aria-hidden="true" />
            )}
            {isApproved ? "Comment access approved" : "Want to comment?"}
          </p>
          <p {...stylex.props(styles.style6)}>{description}</p>
        </div>
        <div {...stylex.props(styles.style30)}>
          {isPending ? (
            <button
              type="button"
              {...stylex.props([
                sharedButtonStyles.base,
                sharedButtonStyles.secondary,
              ])}
              disabled={pending}
              onClick={() => onCancel(request.requestId)}
            >
              Cancel request
            </button>
          ) : isApproved ? (
            <button
              type="button"
              {...stylex.props([
                sharedButtonStyles.base,
                sharedButtonStyles.primary,
              ])}
              onClick={() => window.location.reload()}
            >
              Reload note
            </button>
          ) : (
            <button
              type="button"
              {...stylex.props([
                sharedButtonStyles.base,
                sharedButtonStyles.primary,
              ])}
              disabled={pending}
              onClick={onRequest}
            >
              {pending ? "Requesting…" : "Request comment access"}
            </button>
          )}
        </div>
      </div>
      {error && (
        <p {...stylex.props(styles.style13)} role="status">
          Comment access couldn’t be updated. Try again.
        </p>
      )}
    </div>
  );
}
function ManagerRequests({
  error,
  hasEarlierRequests,
  loading,
  loadingEarlier,
  onLoadEarlier,
  onReview,
  pendingRequestId,
  requests,
}: {
  error: boolean;
  hasEarlierRequests: boolean;
  loading: boolean;
  loadingEarlier: boolean;
  onLoadEarlier: () => void;
  onReview: (
    requestId: string,
    decision: "approved" | "denied",
    capability: SharedNoteCapability,
  ) => void;
  pendingRequestId: string | null;
  requests: Array<{
    capability: SharedNoteCapability;
    entryId: string;
    userEmail: string;
  }>;
}) {
  if (loading) {
    return null;
  }
  if (!requests.length && !hasEarlierRequests && !error) {
    return null;
  }
  return (
    <div {...stylex.props(styles.style27)}>
      <h3 {...stylex.props(styles.style18)}>Access requests</h3>
      {requests.length > 0 && (
        <ul {...stylex.props(styles.style31)}>
          {requests.map((request) => {
            const pending = pendingRequestId === request.entryId;
            return (
              <li key={request.entryId} {...stylex.props(styles.style32)}>
                <div>
                  <p {...stylex.props(styles.style33)}>{request.userEmail}</p>
                  <p {...stylex.props(styles.style34)}>
                    {formatSharedNoteAccessRequestDescription(
                      request.capability,
                    )}
                  </p>
                </div>
                <div {...stylex.props(styles.style35)}>
                  <button
                    type="button"
                    {...stylex.props(
                      sharedButtonStyles.base,
                      sharedButtonStyles.secondary,
                      styles.compactButton,
                    )}
                    disabled={pendingRequestId !== null}
                    onClick={() =>
                      onReview(request.entryId, "denied", request.capability)
                    }
                  >
                    <X {...stylex.props(styles.style36)} aria-hidden="true" />
                    Deny
                  </button>
                  <button
                    type="button"
                    {...stylex.props(
                      sharedButtonStyles.base,
                      sharedButtonStyles.primary,
                      styles.compactButton,
                    )}
                    disabled={pendingRequestId !== null}
                    onClick={() =>
                      onReview(request.entryId, "approved", request.capability)
                    }
                  >
                    {pending ? (
                      <CircleNotch
                        {...stylex.props(styles.style37)}
                        aria-hidden="true"
                      />
                    ) : (
                      <Check
                        {...stylex.props(styles.style36)}
                        aria-hidden="true"
                      />
                    )}
                    Approve
                  </button>
                </div>
              </li>
            );
          })}
        </ul>
      )}
      {hasEarlierRequests && (
        <button
          type="button"
          {...stylex.props(
            sharedButtonStyles.base,
            sharedButtonStyles.secondary,
            styles.loadEarlierRequests,
          )}
          disabled={loadingEarlier}
          onClick={onLoadEarlier}
        >
          {loadingEarlier && (
            <CircleNotch {...stylex.props(styles.style38)} aria-hidden="true" />
          )}
          {loadingEarlier ? "Loading…" : "Load earlier requests"}
        </button>
      )}
      {error && (
        <p {...stylex.props(styles.style13)} role="status">
          Access requests couldn’t be updated. Try again.
        </p>
      )}
    </div>
  );
}
function formatCommentDate(value: string) {
  return new Intl.DateTimeFormat("en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}
